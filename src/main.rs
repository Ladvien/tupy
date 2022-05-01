// https://docs.rs/serial2/latest/serial2/
// https://stackoverflow.com/questions/53440321/how-to-use-serial-port-in-multiple-threads-in-rust
// https://tokio.rs/tokio/tutorial/shared-state
use futures::{stream::StreamExt, SinkExt};
use std::{env, io, str};
use tokio::time::{sleep, Duration};
use tokio_util::codec::{Decoder, Encoder};

use bytes::{BufMut, BytesMut};
use tokio_serial::SerialPortBuilderExt;

#[cfg(unix)]
const DEFAULT_TTY: &str = "/dev/ttyUSB0";
#[cfg(windows)]
const DEFAULT_TTY: &str = "COM1";

struct LineCodec;

impl Decoder for LineCodec {
    type Item = String;
    type Error = io::Error;

    fn decode(&mut self, src: &mut BytesMut) -> Result<Option<Self::Item>, Self::Error> {
        let newline = src.as_ref().iter().position(|b| *b == b'\n');
        if let Some(n) = newline {
            let line = src.split_to(n + 1);
            return match str::from_utf8(line.as_ref()) {
                Ok(s) => Ok(Some(s.to_string())),
                Err(_) => Err(io::Error::new(io::ErrorKind::Other, "Invalid String")),
            };
        }
        Ok(None)
    }
}

impl Encoder<String> for LineCodec {
    type Error = io::Error;

    fn encode(&mut self, item: String, dst: &mut BytesMut) -> Result<(), Self::Error> {
        println!("In writer {:?}", &item);
        dst.reserve(item.len() + 1);
        dst.put(item.as_bytes());
        dst.put_u8(b'\n');
        Ok(())
    }
}

#[tokio::main]
async fn main() -> tokio_serial::Result<()> {
    let mut args = env::args();
    let tty_path = args.nth(1).unwrap_or_else(|| DEFAULT_TTY.into());

    let mut port = tokio_serial::new(tty_path, 115_200).open_native_async()?;

    #[cfg(unix)]
    port.set_exclusive(false)
        .expect("Unable to set serial port exclusive to false");

    let stream = LineCodec.framed(port);
    let (mut tx, mut rx) = stream.split();

    tokio::spawn(async move {
        loop {
            let item = rx
                .next()
                .await
                .expect("Error awaiting future in RX stream.")
                .expect("Reading stream resulted in an error");
            print!("{item}");
        }
    });

    tokio::spawn(async move {
        loop {
            let write_result = tx
                .send(String::from(format!("{}\r", r#"print("hello")"#)))
                .await;
            sleep(Duration::from_secs(2)).await;
            match write_result {
                Ok(_) => (),
                Err(err) => println!("{:?}", err),
            }
        }
    });

    loop {}
}
