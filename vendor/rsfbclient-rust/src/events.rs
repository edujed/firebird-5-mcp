//! Firebird remote events
//!
//! A firebird server never pushes an event notification on the connection that
//! registered it. It asks the client to open a second, "auxiliary" connection
//! and delivers the notifications there, so listening for an event needs three
//! wire operations:
//!
//! * `op_connect_request`, once per attachment, on the main connection: the
//!   server answers with the address it listens on for the auxiliary channel;
//! * `op_que_events`, on the main connection, to register the interest. It is
//!   acknowledged by a plain `op_response`;
//! * `op_event`, pushed by the server on the auxiliary channel, carrying the
//!   updated occurrence counters.
//!
//! A registration is one shot: the server drops it as soon as the notification
//! was delivered, so waiting again means registering again.

use bytes::{Buf, BytesMut};
use std::{
    borrow::Cow,
    io::Read,
    net::{Shutdown, SocketAddr, TcpStream},
    time::Duration,
};

use crate::{
    consts::WireOp,
    wire::{parse_event_notification, EventNotification},
};
use rsfbclient_core::{Charset, FbError};

/// Version tag of an event parameter block (`EPB_version1`)
const EPB_VERSION1: u8 = 1;

/// Maximum length, in bytes, of an encoded firebird event name: the event
/// parameter block stores the length of a name in a single unsigned byte
pub const MAX_EVENT_NAME_LEN: usize = u8::MAX as usize;

/// Maximum size, in bytes, of an event parameter block: firebird refuses to
/// build a block that does not fit in an unsigned short
pub const MAX_EVENT_BLOCK_LEN: usize = u16::MAX as usize;

/// How long to wait for the auxiliary connection to be established.
///
/// The firebird client itself does a plain blocking `connect()` here, but the
/// server only listens for a bounded time: `aux_request` binds the auxiliary
/// port and hands it to `aux_connect`, which `select()`s on it for
/// `port_connect_timeout` seconds before giving up with
/// `isc_net_event_connect_timeout`. That value comes from the
/// `isc_dpb_connect_timeout` dpb item, which this client does not send, so the
/// server falls back to its `ConnectionTimeout` configuration entry, whose
/// default is 180 seconds (`src/common/config/config.h`).
///
/// Waiting any longer would be pointless, the server has stopped listening;
/// waiting less could cut short a connection it would still have accepted.
const AUX_CONNECT_TIMEOUT: Duration = Duration::from_secs(180);

/// Normalize an event name.
///
/// Firebird strips the trailing blanks of an event name, both when a client
/// registers and when the server stores it, so we do it too: this keeps the
/// name we send equal to the name we get back in the notification.
pub fn normalize_event_name(name: &str) -> Result<&str, FbError> {
    let name = name.trim_end_matches(' ');

    if name.is_empty() {
        return Err(FbError::from("A firebird event name cannot be empty"));
    }

    Ok(name)
}

/// Encode an event name with the charset of the connection.
///
/// Event names travel as raw bytes, and the server compares them byte for byte
/// against the names of the `POST_EVENT` statements, which reach it in the
/// connection charset. So a name has to be encoded exactly like the rest of the
/// statements this connection sends, not forced to utf-8.
///
/// The length limit is a limit on those *encoded bytes*, not on rust
/// characters: the block prefixes the name with a single byte of length.
fn encode_event_name<'a>(charset: &Charset, name: &'a str) -> Result<Cow<'a, [u8]>, FbError> {
    let name = normalize_event_name(name)?;
    let encoded = charset.encode(name)?;

    if encoded.len() > MAX_EVENT_NAME_LEN {
        return Err(FbError::from(format!(
            "A firebird event name is limited to {} bytes, but '{}' uses {} once encoded in {}",
            MAX_EVENT_NAME_LEN,
            name,
            encoded.len(),
            charset.on_firebird
        )));
    }

    Ok(encoded)
}

/// Build the event parameter block registering an interest in `events`, a list
/// of event names with the occurrence counter already known by the caller.
///
/// The server notifies the client as soon as the counter it holds for an event
/// reaches the counter provided here, so registering with a counter of zero
/// always fires immediately.
pub fn event_block<'a, I>(charset: &Charset, events: I) -> Result<Vec<u8>, FbError>
where
    I: IntoIterator<Item = (&'a str, u32)>,
{
    let mut epb = vec![EPB_VERSION1];

    for (name, count) in events {
        let name = encode_event_name(charset, name)?;

        epb.push(name.len() as u8);
        epb.extend_from_slice(&name);
        // Counters are little endian, whatever the endianness of the rest of
        // the protocol
        epb.extend_from_slice(&count.to_le_bytes());
    }

    if epb.len() > MAX_EVENT_BLOCK_LEN {
        return Err(FbError::from(format!(
            "The event parameter block is limited to {} bytes, but the requested events need {}",
            MAX_EVENT_BLOCK_LEN,
            epb.len()
        )));
    }

    Ok(epb)
}

/// Parse an event parameter block, returning the occurrence counter of every
/// event name it holds.
///
/// The names are decoded with the charset of the connection, the one they were
/// registered with.
pub fn parse_event_block(charset: &Charset, epb: &[u8]) -> Result<Vec<(String, u32)>, FbError> {
    let (version, mut rest) = epb
        .split_first()
        .ok_or_else(|| FbError::from("Empty event parameter block"))?;

    if *version != EPB_VERSION1 {
        return Err(FbError::from(format!(
            "Unsupported event parameter block version: {}",
            version
        )));
    }

    let mut events = Vec::new();

    while let Some((len, tail)) = rest.split_first() {
        let len = *len as usize;

        if tail.len() < len + 4 {
            return Err(FbError::from("Truncated event parameter block"));
        }

        let (name, tail) = tail.split_at(len);
        let name = charset.decode(name)?;

        let (count, tail) = tail.split_at(4);
        let count = u32::from_le_bytes([count[0], count[1], count[2], count[3]]);

        events.push((name, count));
        rest = tail;
    }

    Ok(events)
}

/// Occurrence counter of `name` in a parsed event parameter block
pub fn event_count(events: &[(String, u32)], name: &str) -> Result<u32, FbError> {
    events
        .iter()
        .find(|(event, _)| event == name)
        .map(|(_, count)| *count)
        .ok_or_else(|| {
            FbError::from(format!(
                "The server notification does not hold the '{}' event",
                name
            ))
        })
}

/// Open the tcp connection of the auxiliary channel.
///
/// Only the connect is bounded by `timeout`; the reads that follow block until
/// an event arrives.
fn connect_aux(addr: SocketAddr, timeout: Duration) -> Result<TcpStream, FbError> {
    let socket = TcpStream::connect_timeout(&addr, timeout)?;

    set_keep_alive(&socket);

    Ok(socket)
}

/// Turn `SO_KEEPALIVE` on, like `setKeepAlive` does in firebird's `inet.cpp`.
///
/// Only that option is set: the idle delay, the probe interval and the probe
/// count are left to the system, exactly as the firebird client leaves them.
///
/// The standard library does not expose the option, so this goes straight to
/// the socket api the process is already linked against rather than pulling a
/// crate in. Failures are ignored on purpose: `aux_connect` ignores them too,
/// keep alive is a nicety and must never stop the events from working.
///
/// Firebird sets the option before connecting; setting it right after makes no
/// practical difference, as the probes only start once the connection has been
/// idle for a while.
#[cfg(unix)]
fn set_keep_alive(socket: &TcpStream) {
    use std::os::{
        fd::AsRawFd,
        raw::{c_int, c_void},
    };

    extern "C" {
        fn setsockopt(
            socket: c_int,
            level: c_int,
            name: c_int,
            value: *const c_void,
            option_len: u32,
        ) -> c_int;
    }

    let (level, name) = keep_alive_option();
    let enable: c_int = 1;

    // SAFETY: `enable` outlives the call, and `option_len` describes it exactly
    unsafe {
        setsockopt(
            socket.as_raw_fd(),
            level,
            name,
            &enable as *const c_int as *const c_void,
            std::mem::size_of::<c_int>() as u32,
        );
    }
}

#[cfg(windows)]
fn set_keep_alive(socket: &TcpStream) {
    use std::os::{
        raw::{c_char, c_int},
        windows::io::AsRawSocket,
    };

    #[link(name = "ws2_32")]
    extern "system" {
        fn setsockopt(
            socket: usize,
            level: c_int,
            name: c_int,
            value: *const c_char,
            option_len: c_int,
        ) -> c_int;
    }

    let (level, name) = keep_alive_option();
    let enable: c_int = 1;

    // SAFETY: `enable` outlives the call, and `option_len` describes it exactly
    unsafe {
        setsockopt(
            socket.as_raw_socket() as usize,
            level,
            name,
            &enable as *const c_int as *const c_char,
            std::mem::size_of::<c_int>() as c_int,
        );
    }
}

#[cfg(not(any(unix, windows)))]
fn set_keep_alive(_socket: &TcpStream) {}

/// `SOL_SOCKET` and `SO_KEEPALIVE`, from `<sys/socket.h>`.
///
/// These are abi constants, they depend on the platform and not on the version
/// of its libc. Linux takes them from `asm-generic/socket.h`, except on the few
/// architectures that kept the historical numbering, which is also the one
/// windows and every other unix use.
#[cfg(any(unix, windows))]
fn keep_alive_option() -> (std::os::raw::c_int, std::os::raw::c_int) {
    if cfg!(all(
        any(target_os = "linux", target_os = "android"),
        not(any(
            target_arch = "mips",
            target_arch = "mips32r6",
            target_arch = "mips64",
            target_arch = "mips64r6",
            target_arch = "sparc",
            target_arch = "sparc64"
        ))
    )) {
        (1, 9)
    } else {
        (0xffff, 0x0008)
    }
}

/// The auxiliary connection a firebird server pushes the `op_event`
/// notifications on
pub struct EventChannel {
    /// Handle of the database the channel was opened for
    db_handle: u32,

    /// Charset of the connection, the one the event names were registered with
    charset: Charset,

    /// Socket the server pushes the notifications on. Unlike the main
    /// connection this one is never encrypted nor compressed: firebird builds
    /// the auxiliary port without carrying over the wire crypt plugin.
    socket: TcpStream,

    /// Data read from the socket but not consumed yet. Kept between reads so a
    /// short read never desyncs the packet framing.
    buff: BytesMut,
}

impl EventChannel {
    /// Open the auxiliary connection.
    ///
    /// `main_peer` is the address of the main connection and `port` the one the
    /// server reported: see [`crate::wire::parse_aux_port`] about why the
    /// address reported by the server is not used.
    pub fn open(
        db_handle: u32,
        charset: Charset,
        main_peer: SocketAddr,
        port: u16,
    ) -> Result<Self, FbError> {
        let mut addr = main_peer;
        addr.set_port(port);

        Ok(Self {
            db_handle,
            charset,
            socket: connect_aux(addr, AUX_CONNECT_TIMEOUT)?,
            buff: BytesMut::with_capacity(256),
        })
    }

    /// Handle of the database the channel was opened for
    pub fn db_handle(&self) -> u32 {
        self.db_handle
    }

    /// Block until the server notifies the registration `event_id`, returning
    /// the occurrence counters it carries.
    ///
    /// Notifications of another registration are skipped: cancelling a
    /// registration races with its delivery, so a stale notification may still
    /// show up on the channel.
    pub fn recv_event(&mut self, event_id: u32) -> Result<Vec<(String, u32)>, FbError> {
        loop {
            let notification = self.recv_notification()?;

            if notification.event_id == event_id {
                return parse_event_block(&self.charset, &notification.epb);
            }
        }
    }

    /// Read the next notification, skipping the keep alive packets
    fn recv_notification(&mut self) -> Result<EventNotification, FbError> {
        loop {
            self.fill(4)?;
            let op_code = self.peek_u32(0);

            if op_code == WireOp::Dummy as u32 {
                // Keep alive packet, it has no body
                self.buff.advance(4);
                continue;
            }

            if op_code == WireOp::Exit as u32 || op_code == WireOp::Disconnect as u32 {
                return Err(FbError::from(
                    "The server closed the event channel while waiting for an event",
                ));
            }

            if op_code != WireOp::Event as u32 {
                return Err(FbError::from(format!(
                    "Unexpected operation {} on the event channel",
                    op_code
                )));
            }

            // The op code, the database handle, and the length of the event
            // parameter block, which is what the length of the packet hangs on
            self.fill(12)?;
            let epb_len = self.peek_u32(8) as usize;

            // Do not let a corrupt length drive the buffer: firebird never
            // builds a block bigger than this
            if epb_len > MAX_EVENT_BLOCK_LEN {
                return Err(FbError::from(format!(
                    "The server announced an event parameter block of {} bytes, over the {} bytes limit",
                    epb_len, MAX_EVENT_BLOCK_LEN
                )));
            }

            // The parameter block is padded to a 4 bytes boundary and followed
            // by the ast routine address, its argument and the registration id
            let len = 12 + epb_len.next_multiple_of(4) + 12;

            self.fill(len)?;
            let mut packet = self.buff.split_to(len).freeze();

            return parse_event_notification(&mut packet);
        }
    }

    /// Read the big endian u32 buffered at `offset`, without consuming it
    fn peek_u32(&self, offset: usize) -> u32 {
        u32::from_be_bytes([
            self.buff[offset],
            self.buff[offset + 1],
            self.buff[offset + 2],
            self.buff[offset + 3],
        ])
    }

    /// Read from the socket until `len` bytes are buffered
    fn fill(&mut self, len: usize) -> Result<(), FbError> {
        let mut chunk = [0; 512];

        while self.buff.len() < len {
            let read = self.socket.read(&mut chunk)?;

            if read == 0 {
                return Err(FbError::from(
                    "The event channel was closed while waiting for an event",
                ));
            }

            self.buff.extend_from_slice(&chunk[..read]);
        }

        Ok(())
    }
}

impl Drop for EventChannel {
    fn drop(&mut self) {
        // Let the server know the auxiliary port is gone instead of leaving it
        // waiting on a half open socket
        let _ = self.socket.shutdown(Shutdown::Both);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::wire::{cancel_events, connect_request, parse_aux_port, que_events};
    use bytes::{BufMut, Bytes};
    use rsfbclient_core::charset::{ASCII, ISO_8859_1, UTF_8, WIN_1252};
    use std::{io::Write, net::TcpListener, thread, time::Instant};

    /// Read `SO_KEEPALIVE` back from the socket, so the tests check the option
    /// really landed and not just that the setter was called. It also proves
    /// the constants of [`keep_alive_option`] are the right ones on whatever
    /// platform the tests are running on.
    #[cfg(unix)]
    fn keep_alive_enabled(socket: &TcpStream) -> bool {
        use std::os::{
            fd::AsRawFd,
            raw::{c_int, c_void},
        };

        extern "C" {
            fn getsockopt(
                socket: c_int,
                level: c_int,
                name: c_int,
                value: *mut c_void,
                option_len: *mut u32,
            ) -> c_int;
        }

        let (level, name) = keep_alive_option();
        let mut enabled: c_int = 0;
        let mut len = std::mem::size_of::<c_int>() as u32;

        // SAFETY: `enabled` and `len` outlive the call, and `len` describes
        // `enabled` exactly
        let res = unsafe {
            getsockopt(
                socket.as_raw_fd(),
                level,
                name,
                &mut enabled as *mut c_int as *mut c_void,
                &mut len,
            )
        };

        assert_eq!(res, 0, "getsockopt(SO_KEEPALIVE) failed");

        enabled != 0
    }

    #[cfg(windows)]
    fn keep_alive_enabled(socket: &TcpStream) -> bool {
        use std::os::{
            raw::{c_char, c_int},
            windows::io::AsRawSocket,
        };

        #[link(name = "ws2_32")]
        extern "system" {
            fn getsockopt(
                socket: usize,
                level: c_int,
                name: c_int,
                value: *mut c_char,
                option_len: *mut c_int,
            ) -> c_int;
        }

        let (level, name) = keep_alive_option();
        let mut enabled: c_int = 0;
        let mut len = std::mem::size_of::<c_int>() as c_int;

        // SAFETY: `enabled` and `len` outlive the call, and `len` describes
        // `enabled` exactly
        let res = unsafe {
            getsockopt(
                socket.as_raw_socket() as usize,
                level,
                name,
                &mut enabled as *mut c_int as *mut c_char,
                &mut len,
            )
        };

        assert_eq!(res, 0, "getsockopt(SO_KEEPALIVE) failed");

        enabled != 0
    }

    /// Build the `op_event` packet a server would push for `events`
    fn event_packet(event_id: u32, events: &[(&str, u32)]) -> Bytes {
        let epb = event_block(&UTF_8, events.iter().copied()).unwrap();

        let mut packet = BytesMut::new();
        packet.put_u32(WireOp::Event as u32);
        packet.put_u32(1); // Database handle
        packet.put_u32(epb.len() as u32);
        packet.put_slice(&epb);
        packet.put_slice(&vec![0; epb.len().next_multiple_of(4) - epb.len()]); // Padding
        packet.put_u32(0); // Ast routine address
        packet.put_u32(0); // Ast routine argument
        packet.put_u32(event_id);

        packet.freeze()
    }

    #[test]
    fn event_block_single_name() {
        let epb = event_block(&UTF_8, [("evento", 0)]).unwrap();

        assert_eq!(
            epb,
            vec![
                1, // EPB_version1
                6, // Name length
                b'e', b'v', b'e', b'n', b't', b'o', //
                0, 0, 0, 0, // Counter, little endian
            ]
        );
    }

    #[test]
    fn event_block_counter_is_little_endian() {
        let epb = event_block(&UTF_8, [("a", 0x0102_0304)]).unwrap();

        assert_eq!(epb, vec![1, 1, b'a', 0x04, 0x03, 0x02, 0x01]);
    }

    #[test]
    fn event_block_multiple_names() {
        let epb = event_block(&UTF_8, [("ab", 1), ("c", 2)]).unwrap();

        assert_eq!(epb, vec![1, 2, b'a', b'b', 1, 0, 0, 0, 1, b'c', 2, 0, 0, 0]);
    }

    #[test]
    fn event_block_strips_trailing_blanks() {
        // Firebird strips them on both ends, so the name we send has to match
        // the name the server sends back
        assert_eq!(
            event_block(&UTF_8, [("ab  ", 0)]).unwrap(),
            event_block(&UTF_8, [("ab", 0)]).unwrap()
        );
    }

    #[test]
    fn event_name_limits_are_counted_in_encoded_bytes() {
        let ascii = "a".repeat(MAX_EVENT_NAME_LEN);
        assert!(encode_event_name(&UTF_8, &ascii).is_ok());

        let too_long = "a".repeat(MAX_EVENT_NAME_LEN + 1);
        assert!(encode_event_name(&UTF_8, &too_long).is_err());

        // The very same name is 256 bytes in utf-8 and 128 in latin 1, so the
        // limit has to be checked after encoding and not on the rust string:
        // rejected on one connection, accepted on the other
        let accented = "é".repeat(128);
        assert_eq!(accented.chars().count(), 128);

        assert_eq!(accented.len(), 256);
        assert!(encode_event_name(&UTF_8, &accented).is_err());

        assert_eq!(
            encode_event_name(&ISO_8859_1, &accented).unwrap().len(),
            128
        );
    }

    #[test]
    fn event_name_cannot_be_empty() {
        assert!(normalize_event_name("").is_err());
        // A name of blanks is empty once the trailing blanks are stripped
        assert!(normalize_event_name("   ").is_err());

        assert!(event_block(&UTF_8, [("", 0)]).is_err());
        assert!(event_block(&ISO_8859_1, [("   ", 0)]).is_err());
    }

    #[test]
    fn event_name_is_encoded_with_the_connection_charset() {
        // 'é' is one byte in latin 1 and two in utf-8. Sending the utf-8 bytes
        // on a latin 1 connection would not match the name the server stored
        // from `POST_EVENT`, which arrived in the connection charset.
        assert_eq!(
            event_block(&ISO_8859_1, [("é", 0)]).unwrap(),
            vec![1, 1, 0xe9, 0, 0, 0, 0]
        );

        assert_eq!(
            event_block(&UTF_8, [("é", 0)]).unwrap(),
            vec![1, 2, 0xc3, 0xa9, 0, 0, 0, 0]
        );
    }

    #[test]
    fn event_name_in_windows_1252() {
        // '€' has no latin 1 encoding, but windows-1252 maps it to 0x80
        assert_eq!(
            event_block(&WIN_1252, [("caf\u{e9}\u{20ac}", 0)]).unwrap(),
            vec![1, 5, b'c', b'a', b'f', 0xe9, 0x80, 0, 0, 0, 0]
        );

        assert!(event_block(&ISO_8859_1, [("\u{20ac}", 0)]).is_err());
    }

    #[test]
    fn event_name_that_the_charset_cannot_represent_is_rejected() {
        let err = event_block(&ASCII, [("é", 0)]).unwrap_err();
        assert!(
            format!("{}", err).contains("ascii"),
            "unexpected error: {}",
            err
        );

        // And it is an error, not a silent replacement
        assert!(event_block(&ISO_8859_1, [("日本", 0)]).is_err());
    }

    #[test]
    fn event_names_roundtrip_through_a_non_utf8_charset() {
        let name = "caf\u{e9}";

        let epb = event_block(&WIN_1252, [(name, 7)]).unwrap();
        let events = parse_event_block(&WIN_1252, &epb).unwrap();

        // The decoded name has to compare equal to the one that was registered,
        // otherwise `event_count` could never find its counter
        assert_eq!(events, vec![(name.to_string(), 7)]);
        assert_eq!(event_count(&events, name).unwrap(), 7);
    }

    #[test]
    fn parse_event_block_rejects_bytes_the_charset_cannot_decode() {
        // 0xff is never a valid utf-8 byte
        assert!(parse_event_block(&UTF_8, &[1, 1, 0xff, 0, 0, 0, 0]).is_err());

        // Anything above 0x7f is out of ascii
        assert!(parse_event_block(&ASCII, &[1, 1, 0xe9, 0, 0, 0, 0]).is_err());

        // The very same byte decodes fine in latin 1, so this is a charset
        // mismatch and not a malformed block
        assert_eq!(
            parse_event_block(&ISO_8859_1, &[1, 1, 0xe9, 0, 0, 0, 0]).unwrap(),
            vec![("é".to_string(), 0)]
        );
    }

    #[test]
    fn event_block_size_is_limited() {
        let name = "a".repeat(MAX_EVENT_NAME_LEN);
        let events = (0..300).map(|_| (name.as_str(), 0)).collect::<Vec<_>>();

        // 300 * (1 + 255 + 4) + 1 is way past the unsigned short firebird uses
        assert!(event_block(&UTF_8, events).is_err());
    }

    #[test]
    fn parse_event_block_roundtrip() {
        let epb = event_block(&UTF_8, [("evento", 3), ("outro", 0)]).unwrap();

        assert_eq!(
            parse_event_block(&UTF_8, &epb).unwrap(),
            vec![("evento".to_string(), 3), ("outro".to_string(), 0)]
        );
    }

    #[test]
    fn parse_event_block_rejects_invalid_input() {
        assert!(parse_event_block(&UTF_8, &[]).is_err());
        // Unknown version
        assert!(parse_event_block(&UTF_8, &[2, 1, b'a', 0, 0, 0, 0]).is_err());
        // Name shorter than announced
        assert!(parse_event_block(&UTF_8, &[1, 6, b'a', 0, 0, 0, 0]).is_err());
        // Missing counter
        assert!(parse_event_block(&UTF_8, &[1, 1, b'a', 0, 0]).is_err());
    }

    #[test]
    fn event_count_of_a_missing_name_is_an_error() {
        let events = vec![("a".to_string(), 7)];

        assert_eq!(event_count(&events, "a").unwrap(), 7);
        assert!(event_count(&events, "b").is_err());
    }

    #[test]
    fn connect_request_layout() {
        assert_eq!(
            connect_request(0x0a0b_0c0d).as_ref(),
            [
                0, 0, 0, 53, // op_connect_request
                0, 0, 0, 1, // P_REQ_async
                0x0a, 0x0b, 0x0c, 0x0d, // Database handle
                0, 0, 0, 0, // Partner identification
            ]
        );
    }

    #[test]
    fn que_events_layout() {
        // A 7 bytes block, so the padding to 4 bytes is exercised
        let epb = event_block(&UTF_8, [("a", 0)]).unwrap();
        assert_eq!(epb.len(), 7);

        assert_eq!(
            que_events(1, &epb, 42).as_ref(),
            [
                0, 0, 0, 48, // op_que_events
                0, 0, 0, 1, // Database handle
                0, 0, 0, 7, // Event parameter block length
                1, 1, b'a', 0, 0, 0, 0, // The block itself
                0, // Padding to a 4 bytes boundary
                0, 0, 0, 0, // Ast routine address
                0, 0, 0, 0, // Ast routine argument
                0, 0, 0, 42, // Event id
            ]
        );
    }

    #[test]
    fn cancel_events_layout() {
        assert_eq!(
            cancel_events(1, 42).as_ref(),
            [
                0, 0, 0, 49, // op_cancel_events
                0, 0, 0, 1, // Database handle
                0, 0, 0, 42, // Event id
            ]
        );
    }

    #[test]
    fn parse_event_notification_reads_the_counters() {
        let mut packet = event_packet(42, &[("evento", 5)]);

        let notification = parse_event_notification(&mut packet).unwrap();
        assert_eq!(notification.event_id, 42);
        assert_eq!(
            parse_event_block(&UTF_8, &notification.epb).unwrap(),
            vec![("evento".to_string(), 5)]
        );
        // The whole packet was consumed, padding included
        assert!(packet.is_empty());
    }

    #[test]
    fn parse_event_notification_of_an_unpadded_block() {
        // A 4 bytes aligned block adds no padding at all
        let epb = event_block(&UTF_8, [("ab", 0)]).unwrap();
        assert_eq!(epb.len(), 8);

        let mut packet = event_packet(1, &[("ab", 0)]);
        let notification = parse_event_notification(&mut packet).unwrap();

        assert_eq!(
            parse_event_block(&UTF_8, &notification.epb).unwrap(),
            vec![("ab".to_string(), 0)]
        );
        assert!(packet.is_empty());
    }

    #[test]
    fn parse_event_notification_rejects_another_operation() {
        let mut packet = Bytes::from_static(&[0, 0, 0, 9]);

        assert!(parse_event_notification(&mut packet).is_err());
    }

    #[test]
    fn aux_port_is_read_in_network_byte_order() {
        // sockaddr_in of 192.168.1.10:32771, as a little endian host writes it
        let sockaddr = [
            2, 0, // sin_family, host byte order
            0x80, 0x03, // sin_port, network byte order
            192, 168, 1, 10, // sin_addr
            0, 0, 0, 0, 0, 0, 0, 0, // sin_zero
        ];

        assert_eq!(parse_aux_port(&sockaddr).unwrap(), 0x8003);
    }

    #[test]
    fn aux_port_ignores_the_address_family_layout() {
        // A macOS server writes sa_len then sa_family, but the port stays at
        // the same offset, in the same byte order
        let macos = [16, 2, 0x0b, 0xea, 127, 0, 0, 1];
        let posix = [2, 0, 0x0b, 0xea, 127, 0, 0, 1];

        assert_eq!(parse_aux_port(&macos).unwrap(), 3050);
        assert_eq!(parse_aux_port(&posix).unwrap(), 3050);
    }

    #[test]
    fn aux_port_rejects_invalid_data() {
        assert!(parse_aux_port(&[]).is_err());
        assert!(parse_aux_port(&[2, 0, 1]).is_err());
        // A port of zero means the server failed to listen
        assert!(parse_aux_port(&[2, 0, 0, 0, 127, 0, 0, 1]).is_err());
    }

    /// The connect timeout is not a value we get to pick: it is the window the
    /// server keeps the auxiliary port open, see `AUX_CONNECT_TIMEOUT`
    #[test]
    fn aux_connect_timeout_is_the_firebird_connection_timeout() {
        assert_eq!(AUX_CONNECT_TIMEOUT, Duration::from_secs(180));
    }

    #[test]
    #[cfg(any(unix, windows))]
    fn aux_socket_enables_keep_alive() {
        let listener = TcpListener::bind(("127.0.0.1", 0)).unwrap();
        let addr = listener.local_addr().unwrap();

        // Off by default, so the assert below cannot pass by accident
        let plain = TcpStream::connect(addr).unwrap();
        assert!(!keep_alive_enabled(&plain));

        let stream = connect_aux(addr, AUX_CONNECT_TIMEOUT).unwrap();
        assert!(keep_alive_enabled(&stream));
    }

    /// `recv_event` blocks on the socket until an event arrives: a non blocking
    /// mode left over from `connect_timeout` would turn every quiet moment
    /// into an error
    #[test]
    fn aux_socket_is_blocking() {
        let listener = TcpListener::bind(("127.0.0.1", 0)).unwrap();
        let addr = listener.local_addr().unwrap();

        let mut stream = connect_aux(addr, AUX_CONNECT_TIMEOUT).unwrap();
        let (mut server, _) = listener.accept().unwrap();

        let reader = thread::spawn(move || {
            let mut byte = [0; 1];
            stream.read_exact(&mut byte).map(|_| byte[0])
        });

        // A non blocking socket would have given up with WouldBlock by now
        thread::sleep(Duration::from_millis(200));
        assert!(!reader.is_finished());

        server.write_all(&[42]).unwrap();
        assert_eq!(reader.join().unwrap().unwrap(), 42);
    }

    /// Bounding the connect must not slow down or break a connect that works
    #[test]
    fn aux_connect_succeeds_within_the_timeout() {
        let listener = TcpListener::bind(("127.0.0.1", 0)).unwrap();
        let addr = listener.local_addr().unwrap();

        let start = Instant::now();
        let stream = connect_aux(addr, Duration::from_secs(30)).unwrap();

        assert_eq!(stream.peer_addr().unwrap(), addr);
        assert!(start.elapsed() < Duration::from_secs(5));
    }

    /// A refused connection has to surface right away instead of waiting for
    /// the whole timeout
    #[test]
    fn aux_connect_reports_a_refused_connection() {
        // Bind then drop, so the port is almost certainly free and refusing
        let addr = {
            let listener = TcpListener::bind(("127.0.0.1", 0)).unwrap();
            listener.local_addr().unwrap()
        };

        let start = Instant::now();
        assert!(connect_aux(addr, AUX_CONNECT_TIMEOUT).is_err());
        assert!(start.elapsed() < Duration::from_secs(5));
    }
}
