# Kayak
Kayak is a prototype for proactive-adaptive arbitration between shipping compute and shipping data. Kayak is a fork of [Splinter](https://github.com/utah-scs/splinter).

## Setting up Kayak
To run Kayak on [CloudLab](https://www.cloudlab.us/) using `xl170` machine, follow these steps:
1. Instantiate a CloudLab cluster using the python profile `cloudlab_profile.py`. This should provide a cluster of bare-metal machines running Ubuntu 18.04 with kernel 4.15.0-147 and gcc 7.5.0.
1. Install MLNX OFED driver. For compatibility, check [here](https://www.mellanox.com/support/mlnx-ofed-matrix?mtag=linux_sw_drivers). The [LTS version](https://content.mellanox.com/ofed/MLNX_OFED-4.9-2.2.4.0/MLNX_OFED_LINUX-4.9-2.2.4.0-ubuntu18.04-x86_64.tgz) is recommended.
2. Run the provided `setup.sh`. This will install DPDK as well as the latest version of Rust.
3. `make`.
4. Create `splinter/client.toml` for client configuration and `db/server.toml` for server configuration.

You can also refer to the README.md of splinter [here](https://github.com/utah-scs/splinter/blob/master/README.md) or [here](README_Splinter.md).

### Running Baseline
To run the YCSB-T workload:
Inside `splinter/`, run
`sudo env RUST_LOG=debug LD_LIBRARY_PATH=../net/target/native ./target/release/ycsb-t`

To run the synthetic workload:
Inside `splinter/`, run
`sudo env RUST_LOG=debug LD_LIBRARY_PATH=../net/target/native ./target/release/pushback`


### Running Kayak
To run the YCSB-T workload: 
Inside `splinter/`, run
`sudo env RUST_LOG=debug LD_LIBRARY_PATH=../net/target/native ./target/release/ycsb-t-kayak`

To run the synthetic workload:
Inside `splinter/`, run
`sudo env RUST_LOG=debug LD_LIBRARY_PATH=../net/target/native ./target/release/pushback-kayak`



## Notes
please consider to cite our paper if you use the code or data in your research project.
```
@inproceedings{kayak-nsdi21,
  author    = {Jie You and Jingfeng Wu and Xin Jin and Mosharaf Chowdhury},
  booktitle = {USENIX NSDI},
  title     = {Ship Compute or Ship Data? Why Not Both?},
  year      = {2021}
}
```

## Acknowledgements

Thanks to Chinmay Kulkarni, Ankit Bhardwaj and Ryan Stutsman for the [Splinter repo](https://github.com/utah-scs/splinter).

## Contact
Jie You (jieyou@umich.edu)
