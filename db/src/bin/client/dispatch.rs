/* Copyright (c) 2018 University of Utah
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

use std::cell::Cell;
use std::fmt::Display;
use std::net::Ipv4Addr;
use std::str::FromStr;

use db::config;
use db::e2d2::allocators::*;
use db::e2d2::common::EmptyMetadata;
use db::e2d2::headers::*;
use db::e2d2::interface::*;
use db::log::*;
use db::rpc;

/// A simple RPC request generator for Sandstorm.
pub struct Sender {
    // The network interface over which requests will be sent out.
    net_port: CacheAligned<PortQueue>,

    // The UDP header on each packet generated by the request generator.
    req_udp_header: UdpHeader,

    // The IP header on each packet generated by the request generator.
    // Currently using IPv4.
    req_ip_header: IpHeader,

    // The MAC header on each packet generated by the request generator.
    req_mac_header: MacHeader,

    // Tracks number of packets sent to the server for occasional debug messages.
    requests_sent: Cell<u64>,

    // The number of destination UDP ports a packet can be sent to.
    dst_ports: u16,
}

impl Sender {
    /// Constructs a Sender.
    ///
    /// The RPC, UDP, IP, and MAC headers on packets generated by this instance are pre-computed
    /// in this method.
    ///
    /// # Arguments
    ///
    /// * `config`:    Network related configuration such as the MAC and IP address.
    /// * `port`:      Network port on which packets will be sent.
    /// * `dst_ports`: The number of destination UDP ports a packet can be sent to.
    ///
    /// # Return
    ///
    /// A Sender that can be used to send RPC requests to a Sandstorm server.
    pub fn new(
        config: &config::ClientConfig,
        port: CacheAligned<PortQueue>,
        dst_ports: u16,
    ) -> Sender {
        // Create UDP, IP, and MAC headers that are placed on all outgoing packets.
        // Length fields are tweaked on a request-by-request basis in the outgoing
        // packets.
        let mut udp_header: UdpHeader = UdpHeader::new();
        udp_header.set_src_port(port.txq() as u16);
        udp_header.set_dst_port(0);
        udp_header.set_length(8);
        udp_header.set_checksum(0);

        // Create a common ip header.
        let ip_src_addr: u32 =
            u32::from(Ipv4Addr::from_str(&config.ip_address).expect("Failed to create source IP."));
        let ip_dst_addr: u32 = u32::from(
            Ipv4Addr::from_str(&config.server_ip_address)
                .expect("Failed to create destination IP."),
        );

        let mut ip_header: IpHeader = IpHeader::new();
        ip_header.set_src(ip_src_addr);
        ip_header.set_dst(ip_dst_addr);
        ip_header.set_ttl(128);
        ip_header.set_version(4);
        ip_header.set_ihl(5);
        ip_header.set_length(20);
        ip_header.set_protocol(0x11);

        // Create a common mac header.
        let mut mac_header: MacHeader = MacHeader::new();
        mac_header.src = config.parse_mac();
        mac_header.dst = config.parse_server_mac();
        mac_header.set_etype(0x0800);

        Sender {
            net_port: port.clone(),
            req_udp_header: udp_header,
            req_ip_header: ip_header,
            req_mac_header: mac_header,
            requests_sent: Cell::new(0),
            dst_ports: dst_ports,
        }
    }

    /// Creates and sends out a get() RPC request. Network headers are populated based on arguments
    /// passed into new() above.
    ///
    /// # Arguments
    ///
    /// * `tenant`: Id of the tenant requesting the item.
    /// * `table`:  Id of the table from which the key is looked up.
    /// * `key`:    Byte string of key whose value is to be fetched. Limit 64 KB.
    /// * `id`:     RPC identifier.
    #[allow(dead_code)]
    pub fn send_get(&self, tenant: u32, table: u64, key: &[u8], id: u64) {
        let request = rpc::create_get_rpc(
            &self.req_mac_header,
            &self.req_ip_header,
            &self.req_udp_header,
            tenant,
            table,
            key,
            id,
            self.get_dst_port(tenant),
        );

        self.send_req(request);
    }

    /// Creates and sends out a put() RPC request. Network headers are populated based on arguments
    /// passed into new() above.
    ///
    /// # Arguments
    ///
    /// * `tenant`: Id of the tenant requesting the insertion.
    /// * `table`:  Id of the table into which the key-value pair is to be inserted.
    /// * `key`:    Byte string of key whose value is to be inserted. Limit 64 KB.
    /// * `val`:    Byte string of the value to be inserted.
    /// * `id`:     RPC identifier.
    #[allow(dead_code)]
    pub fn send_put(&self, tenant: u32, table: u64, key: &[u8], val: &[u8], id: u64) {
        let request = rpc::create_put_rpc(
            &self.req_mac_header,
            &self.req_ip_header,
            &self.req_udp_header,
            tenant,
            table,
            key,
            val,
            id,
            self.get_dst_port(tenant),
        );

        self.send_req(request);
    }

    /// Creates and sends out a multiget() RPC request. Network headers are populated based on
    /// arguments passed into new() above.
    ///
    /// # Arguments
    ///
    /// * `tenant`: Id of the tenant requesting the item.
    /// * `table`:  Id of the table from which the key is looked up.
    /// * `k_len`:  The length of each key to be looked up at the server. All keys are
    ///               assumed to be of equal length.
    /// * `n_keys`: The number of keys to be looked up at the server.
    /// * `keys`:   Byte string of keys whose values are to be fetched.
    /// * `id`:     RPC identifier.
    #[allow(dead_code)]
    pub fn send_multiget(
        &self,
        tenant: u32,
        table: u64,
        k_len: u16,
        n_keys: u32,
        keys: &[u8],
        id: u64,
    ) {
        let request = rpc::create_multiget_rpc(
            &self.req_mac_header,
            &self.req_ip_header,
            &self.req_udp_header,
            tenant,
            table,
            k_len,
            n_keys,
            keys,
            id,
            self.get_dst_port(tenant),
        );

        self.send_req(request);
    }

    /// Creates and sends out an invoke() RPC request. Network headers are populated based on
    /// arguments passed into new() above.
    ///
    /// # Arguments
    ///
    /// * `tenant`:   Id of the tenant requesting the invocation.
    /// * `name_len`: The number of bytes at the head of the payload corresponding to the
    ///               extensions name.
    /// * `payload`:  The RPC payload to be written into the packet. Must contain the name of the
    ///               extension followed by it's arguments.
    /// * `id`:       RPC identifier.
    pub fn send_invoke(&self, tenant: u32, name_len: u32, payload: &[u8], id: u64) {
        let request = rpc::create_invoke_rpc(
            &self.req_mac_header,
            &self.req_ip_header,
            &self.req_udp_header,
            tenant,
            name_len,
            payload,
            id,
            self.get_dst_port(tenant),
            // (id & 0xffff) as u16 & (self.dst_ports - 1),
        );

        self.send_req(request);
    }

    /// Computes the destination UDP port given a tenant identifier.
    #[inline]
    fn get_dst_port(&self, tenant: u32) -> u16 {
        // The two least significant bytes of the tenant id % the total number of destination
        // ports.
        (tenant & 0xffff) as u16 & (self.dst_ports - 1)
    }

    /// Sends a request/packet parsed upto IP out the network interface.
    #[inline]
    fn send_req(&self, request: Packet<IpHeader, EmptyMetadata>) {
        // Send the request out the network.
        unsafe {
            let mut pkts = [request.get_mbuf()];

            let sent = self.net_port
                .send(&mut pkts)
                .expect("Failed to send packet!");

            if sent < pkts.len() as u32 {
                warn!("Failed to send all packets!");
            }
        }

        // Update the number of requests sent out by this generator.
        let r = self.requests_sent.get();
        if r & 0xffffff == 0 {
            info!("Sent many requests...");
        }
        self.requests_sent.set(r + 1);
    }
}

/// A Receiver of responses to RPC requests.
pub struct Receiver<T>
where
    T: PacketTx + PacketRx + Display + Clone + 'static,
{
    // The network interface over which responses will be received from.
    net_port: T,

    // The maximum number of packets that can be received from the network port in one shot.
    max_rx_packets: u32,

    // The total number of responses received.
    responses_recv: Cell<u64>,
}

// Implementation of methods on Receiver.
impl<T> Receiver<T>
where
    T: PacketTx + PacketRx + Display + Clone + 'static,
{
    /// Constructs a Receiver.
    ///
    /// # Arguments
    ///
    /// * `port`:   Network port on which packets will be received.
    ///
    /// # Return
    ///
    /// A Receiver capable of receiving RPC responses over the network.
    pub fn new(port: T) -> Receiver<T> {
        Receiver {
            net_port: port.clone(),
            max_rx_packets: 32,
            responses_recv: Cell::new(0),
        }
    }

    /// Receives responses/packets from the network interface.
    #[inline]
    pub fn recv_res(&self) -> Option<Vec<Packet<UdpHeader, EmptyMetadata>>> {
        // Allocate a vector of mutable MBuf pointers into which raw packets will be received.
        let mut mbuf_vector = Vec::with_capacity(self.max_rx_packets as usize);

        // This unsafe block is needed in order to populate mbuf_vector with a bunch of pointers,
        // and subsequently manipulate these pointers. DPDK will take care of assigning these to
        // actual MBuf's.
        unsafe {
            mbuf_vector.set_len(self.max_rx_packets as usize);

            // Try to receive packets from the network port.
            let recvd = self.net_port
                .recv(&mut mbuf_vector[..])
                .expect("Error on packet receive") as usize;

            // Return if packets weren't available for receive.
            if recvd == 0 {
                return None;
            }

            // Update the number of responses received.
            let r = self.responses_recv.get();
            if r & 0xffffff == 0 {
                info!("Received many responses...");
            }
            self.responses_recv.set(r + 1);

            // Clear out any dangling pointers in mbuf_vector.
            mbuf_vector.drain(recvd..self.max_rx_packets as usize);

            // Vector to hold packets parsed from mbufs.
            let mut packets = Vec::with_capacity(mbuf_vector.len());

            // Wrap up the received Mbuf's into Packets. The refcount on the mbuf's were set by
            // DPDK, and do not need to be bumped up here. Hence, the call to
            // packet_from_mbuf_no_increment().
            for mbuf in mbuf_vector.iter_mut() {
                let packet = packet_from_mbuf_no_increment(*mbuf, 0)
                    .parse_header::<MacHeader>()
                    .parse_header::<IpHeader>()
                    .parse_header::<UdpHeader>();

                packets.push(packet);
            }

            return Some(packets);
        }
    }
}
