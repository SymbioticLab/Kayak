/* Copyright (c) 2017 University of Utah
 *
 * Permission to use, copy, modify, and distribute this software for any
 * purpose with or without fee is hereby granted, provided that the above
 * copyright notice and this permission notice appear in all copies.
 *
 * THE SOFTWARE IS PROVIDED "AS IS" AND THE AUTHOR(S) DISCLAIM ALL WARRANTIES
 * WITH REGARD TO THIS SOFTWARE INCLUDING ALL IMPLIED WARRANTIES OF
 * MERCHANTABILITY AND FITNESS. IN NO EVENT SHALL AUTHORS BE LIABLE FOR
 * ANY SPECIAL, DIRECT, INDIRECT, OR CONSEQUENTIAL DAMAGES OR ANY DAMAGES
 * WHATSOEVER RESULTING FROM LOSS OF USE, DATA OR PROFITS, WHETHER IN AN
 * ACTION OF CONTRACT, NEGLIGENCE OR OTHER TORTIOUS ACTION, ARISING OUT OF
 * OR IN CONNECTION WITH THE USE OR PERFORMANCE OF THIS SOFTWARE.
 */

#![feature(use_extern_macros)]

extern crate db;

use std::sync::Arc;
use std::fmt::Display;

use db::log::*;

use db::e2d2::scheduler::*;
use db::e2d2::interface::*;
use db::e2d2::scheduler::NetBricksContext as NetbricksContext;
use db::e2d2::config::{NetbricksConfiguration, PortConfiguration};

use db::server_dispatch::ServerDispatch;

/// This function sets up a Sandstorm server's dispatch thread on top
/// of Netbricks.
fn setup_server<T, S>(ports: Vec<T>, scheduler: &mut S)
where
    T: PacketTx + PacketRx + Display + Clone + 'static,
    S: Scheduler + Sized,
{
    if ports.len() != 1 {
        error!("Server should be configured with exactly 1 port!");
        std::process::exit(1);
    }

    let server: ServerDispatch<T> = ServerDispatch::new(ports[0].clone());

    // Add the server to a netbricks pipeline.
    match scheduler.add_task(server) {
        Ok(_) => {
            info!("Successfully added server to a Netbricks pipeline.");
        }

        Err(ref err) => {
            error!("Error while adding to Netbricks pipeline {}", err);
            std::process::exit(1);
        }
    }
}

/// Returns a struct of type NetbricksConfiguration which can be used to
/// initialize Netbricks with a default set of parameters.
///
/// If used to initialize Netbricks, this struct will run the parent server
/// thread on core 0, and one scheduler on core 1. Packet buffers will be
/// allocated from a 2 GB memory pool, with 64 MB cached at core 1. DPDK will
/// be initialized as a primary process without any additional arguments. A
/// single network interface/port with 1 transmit queue, 1 receive queue, 256
/// receive descriptors, and 256 transmit descriptors will be made available to
/// Netbricks. Loopback, hardware transmit segementation offload, and hardware
/// checksum offload will be disabled on this port.
fn get_default_netbricks_config() -> NetbricksConfiguration {
    // General arguments supplied to netbricks.
    let net_config_name = String::from("server");
    let dpdk_secondary: bool = false;
    let net_primary_core: i32 = 0;
    let net_cores: Vec<i32> = vec![1];
    let net_strict_cores: bool = true;
    let net_pool_size: u32 = 2048 - 1;
    let net_cache_size: u32 = 64;
    let net_dpdk_args: Option<String> = None;

    // Port configuration. Required to configure the physical network interface.
    let net_port_name = String::from("0000:04:00.1");
    let net_port_rx_queues: Vec<i32> = net_cores.clone();
    let net_port_tx_queues: Vec<i32> = net_cores.clone();
    let net_port_rxd: i32 = 256;
    let net_port_txd: i32 = 256;
    let net_port_loopback: bool = false;
    let net_port_tcp_tso: bool = false;
    let net_port_csum_offload: bool = false;

    let net_port_config = PortConfiguration {
                              name: net_port_name,
                              rx_queues: net_port_rx_queues,
                              tx_queues: net_port_tx_queues,
                              rxd: net_port_rxd,
                              txd: net_port_txd,
                              loopback: net_port_loopback,
                              tso: net_port_tcp_tso,
                              csum: net_port_csum_offload,
                          };

    // The set of ports used by netbricks.
    let net_ports: Vec<PortConfiguration> = vec![net_port_config];

    NetbricksConfiguration {
        name: net_config_name,
        secondary: dpdk_secondary,
        primary_core: net_primary_core,
        cores: net_cores,
        strict: net_strict_cores,
        ports: net_ports,
        pool_size: net_pool_size,
        cache_size: net_cache_size,
        dpdk_args: net_dpdk_args,
    }
}

/// This function configures and initializes Netbricks. In the case of a
/// failure, it causes the program to exit.
///
/// Returns a Netbricks context which can be used to setup and start the
/// server/client.
fn config_and_init_netbricks() -> NetbricksContext {
    let net_config: NetbricksConfiguration = get_default_netbricks_config();

    // Initialize Netbricks and return a handle.
    match initialize_system(&net_config) {
        Ok(net_context) => {
            return net_context;
        }

        Err(ref err) => {
            error!("Error during Netbricks init: {}", err);
            // TODO: Drop NetbricksConfiguration?
            std::process::exit(1);
        }
    }
}

fn main() {
    // Basic setup and initialization.
    db::env_logger::init()
                .expect("ERROR: failed to initialize logger!");
    info!("Starting up Sandstorm server.");

    // Setup Netbricks.
    let mut net_context: NetbricksContext = config_and_init_netbricks();

    // Setup the server pipeline.
    net_context.start_schedulers();
    net_context.add_pipeline_to_run(Arc::new(
            move | ports, scheduler: &mut StandaloneScheduler |
            setup_server(ports, scheduler)
            ));

    // Run the server.
    net_context.execute();

    loop {
        ;
    }

    // Stop the server.
    // net_context.stop();
}