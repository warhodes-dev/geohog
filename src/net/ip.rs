//! Filters undesirable ip addresses like 0.0.0.0 or 127.0.0.1
//! 
//! Current network
//!     0.0.0.0 - 0.255.255.255
//!     127.0.0.0 - 127.255.255.255
//! 
//! Non routable address ranges:
//!     10.0.0.0 - 10.255.255.255
//!     172.16.0.0 - 172.31.255.255
//!     192.168.0.0 - 192.168.255.255
//! 
//! Multicast IP addresses:
//!     224.0.0.0 - 239.255.255.255
//! 
//! Carrier-grade NAT deployment
//!     100.64.0.0 - 100.127.255.255
//! 
//! 