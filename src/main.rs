use sntpc::{Error, NtpContext, NtpTimestampGenerator, NtpUdpSocket, Result};
use std::net::{SocketAddr, ToSocketAddrs, UdpSocket};
use std::{thread, time};

use warp::{Filter, http::Response, reply};
use warp::reply::json;

// StdTimestampGen is a simple implementation of NtpTimestampGenerator
#[derive(Copy, Clone, Default)]
struct StdTimestampGen {
    duration: time::Duration,
}

// Time is a simple struct to hold NTP server time values
#[derive(Clone, Copy, Debug)]
struct Time {
    /// NTP server seconds value
    pub seconds: u32,
    /// NTP server seconds fraction value (microseconds)
    pub seconds_fraction: u32,
    /// Request roundtrip time in microseconds
    pub roundtrip: u64,
    /// Offset of the current system time with one received from a NTP server in microseconds
    pub offset: i64,
}

// Implement NtpTimestampGenerator for StdTimestampGen
impl NtpTimestampGenerator for StdTimestampGen {
    fn init(&mut self) {
        self.duration = std::time::SystemTime::now()
            .duration_since(std::time::SystemTime::UNIX_EPOCH)
            .unwrap();
    }

    fn timestamp_sec(&self) -> u64 {
        self.duration.as_secs()
    }

    fn timestamp_subsec_micros(&self) -> u32 {
        self.duration.subsec_micros()
    }
}


// UdpSocketWrapper is a simple wrapper around UdpSocket to implement NtpUdpSocket
#[derive(Debug)]
struct UdpSocketWrapper(UdpSocket);

// NtpUdpSocket implementation for UdpSocketWrapper
impl NtpUdpSocket for &UdpSocketWrapper {
    fn send_to<T: ToSocketAddrs>(
        &self,
        buf: &[u8],
        addr: T,
    ) -> Result<usize> {
        match self.0.send_to(buf, addr) {
            Ok(usize) => Ok(usize),
            Err(_) => Err(Error::Network),
        }
    }

    fn recv_from(&self, buf: &mut [u8]) -> Result<(usize, SocketAddr)> {
        match self.0.recv_from(buf) {
            Ok((size, addr)) => Ok((size, addr)),
            Err(_) => Err(Error::Network),
        }
    }
}

// Main function
#[tokio::main]
async fn main() {
    let mut timer = Time {
        seconds: 0, seconds_fraction: 0, roundtrip: 0, offset: 0
    };

    thread::spawn(move || {
        ntp_loop(&mut timer)
    });

    let resp = warp::any().map(move || {
        json(&String::from(format!("NTP time: {}.{:06}", timer.seconds, timer.seconds_fraction)))
    });

    let routes = warp::get().and(
        resp
    );

    warp::serve(routes)
        .run(([127, 0, 0, 1], 3030))
        .await
}

// ntp_loop is a simple function to send NTP requests in a loop
fn ntp_loop(timer: &mut Time) -> Result<()> {
    let socket =
        UdpSocket::bind("0.0.0.0:7777").expect("Unable to crate UDP socket");
    socket
        .set_read_timeout(Some(time::Duration::from_secs(2)))
        .expect("Unable to set UDP socket read timeout");
    let sock_wrapper = UdpSocketWrapper(socket);
    let duration = time::Duration::from_secs(10);

    loop {
        req(&sock_wrapper, duration, timer);
    }
}

// req is a simple function to send a single NTP request
fn req(sock_wrapper: &UdpSocketWrapper, duration: time::Duration, timer: &mut Time) {
    let ntp_context = NtpContext::new(StdTimestampGen::default());
    let result =
        sntpc::get_time("time.google.com:123", sock_wrapper, ntp_context);
    match result {
        Ok(time) => {
            timer.seconds = time.sec();
            timer.seconds_fraction = time.sec_fraction();
            timer.roundtrip = time.roundtrip();
            timer.offset = time.offset();
        }
        Err(err) => println!("Err: {:?}", err),
    }

    thread::sleep(duration);
}