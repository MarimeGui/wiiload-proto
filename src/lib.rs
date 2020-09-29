use miniz_oxide::deflate::compress_to_vec;
use std::convert::TryInto;
use std::io::Error as IOError;
use std::io::Write;
use std::net::TcpStream;

const PROTOCOL_VERSION_MAJOR: u8 = 0;
const PROTOCOL_VERSION_MINOR: u8 = 5;

/// All errors net_send can throw
pub enum WiiLoadFail {
    ArgsTooLong,
    BinaryTooLong,
    NetError(IOError),
}

impl From<IOError> for WiiLoadFail {
    fn from(r: IOError) -> WiiLoadFail {
        WiiLoadFail::NetError(r)
    }
}

struct NetworkPacketHeader {
    args_len: u16,
    buffer_size: u32,
    uncompressed_buffer_size: u32,
}

impl NetworkPacketHeader {
    const fn as_u8_buf(&self) -> [u8; 16] {
        [
            // Magic Number
            b'H',
            b'A',
            b'X',
            b'X',
            // Version number
            PROTOCOL_VERSION_MAJOR,
            PROTOCOL_VERSION_MINOR,
            // Args length
            ((self.args_len >> 8) & 0xFF) as u8,
            (self.args_len & 0xFF) as u8,
            // Sent buffer size
            ((self.buffer_size >> 24) & 0xFF) as u8,
            ((self.buffer_size >> 16) & 0xFF) as u8,
            ((self.buffer_size >> 8) & 0xFF) as u8,
            (self.buffer_size & 0xFF) as u8,
            // Uncompressed buffer size
            ((self.uncompressed_buffer_size >> 24) & 0xFF) as u8,
            ((self.uncompressed_buffer_size >> 16) & 0xFF) as u8,
            ((self.uncompressed_buffer_size >> 8) & 0xFF) as u8,
            (self.uncompressed_buffer_size & 0xFF) as u8,
        ]
    }
}

/// Main function of this library that sends the binary to the Wii
/// Compression refers to the compression level used. If None, the binary will be sent directly, bypassing any compressor. If Some(v), v should be between 0 and 10.
pub fn net_send(
    connected_wii: &mut TcpStream,
    executable_binary: &[u8],
    arguments: String,
    compression: Option<u8>,
) -> Result<(), WiiLoadFail> {
    // ---- Process stuff ----

    // Get arg length and fail if necessary
    let args_len = match TryInto::<u16>::try_into(arguments.as_bytes().len()) {
        Ok(v) => v,
        Err(_) => return Err(WiiLoadFail::ArgsTooLong),
    };

    // Check Uncompressed size if too long
    let executable_binary_len = match TryInto::<u32>::try_into(executable_binary.len()) {
        Ok(v) => v,
        Err(_) => return Err(WiiLoadFail::BinaryTooLong),
    };

    // Hack to extend lifetime of compressed Vec
    let maybe_compressed = match compression {
        Some(v) => compress_to_vec(executable_binary, v),
        None => Vec::new(),
    };

    // Get slice and length to send over
    let (processed_executable_binary, processed_executable_binary_len) = match compression {
        Some(_) => {
            // Get Vec from hack
            let compressed_executable_binary = maybe_compressed.as_slice();
            // Check compressed length
            let compressed_executable_binary_len =
                match TryInto::<u32>::try_into(compressed_executable_binary.len()) {
                    Ok(v) => v,
                    Err(_) => return Err(WiiLoadFail::BinaryTooLong),
                };
            (
                compressed_executable_binary,
                compressed_executable_binary_len,
            )
        }
        None => (executable_binary, executable_binary_len),
    };

    // Make packet header
    let transfer_header = NetworkPacketHeader {
        args_len,
        buffer_size: processed_executable_binary_len,
        uncompressed_buffer_size: executable_binary_len,
    };

    // ---- Send to Wii ----
    connected_wii.write_all(&transfer_header.as_u8_buf())?;
    connected_wii.write_all(processed_executable_binary)?;
    connected_wii.write_all(arguments.as_bytes())?;

    Ok(())
}
