// https://github.com/dhylands/serial-monitor/blob/master/src/main.rs
// https://docs.rs/serial2/latest/serial2/
// https://stackoverflow.com/questions/53440321/how-to-use-serial-port-in-multiple-threads-in-rust
// https://tokio.rs/tokio/tutorial/shared-state
use futures::{stream::StreamExt, FutureExt, SinkExt};
use std::{env, io, str};
use tokio::time::{sleep, Duration};
use tokio_util::codec::{Decoder, Encoder};

use bytes::{BufMut, Bytes, BytesMut};
use tokio_serial::SerialPortBuilderExt;

use crossterm::{
    event::{Event, EventStream, KeyCode, KeyEvent, KeyModifiers},
    terminal::{disable_raw_mode, enable_raw_mode},
};

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
        // println!("In writer {:?}", &item);
        dst.reserve(item.len() + 1);
        dst.put(item.as_bytes());
        dst.put_u8(b'\n');
        Ok(())
    }
}

pub fn handle_key_press(key_event: KeyEvent) -> Option<Bytes> {
    let mut buf = [0; 4];

    let key_str: Option<&[u8]> = match key_event.code {
        KeyCode::Backspace => Some(b"\x08"),
        KeyCode::Enter => Some(b"\r"),
        KeyCode::Left => Some(b"\x1b[D"),
        KeyCode::Right => Some(b"\x1b[C"),
        KeyCode::Home => Some(b"\x1b[H"),
        KeyCode::End => Some(b"\x1b[F"),
        KeyCode::Up => Some(b"\x1b[A"),
        KeyCode::Down => Some(b"\x1b[B"),
        KeyCode::Tab => Some(b"\x09"),
        KeyCode::Delete => Some(b"\x1b[3~"),
        KeyCode::Insert => Some(b"\x1b[2~"),
        KeyCode::Esc => Some(b"\x1b"),
        KeyCode::Char(ch) => {
            if key_event.modifiers & KeyModifiers::CONTROL == KeyModifiers::CONTROL {
                buf[0] = ch as u8;
                if (ch >= 'a' && ch <= 'z') || (ch == ' ') {
                    buf[0] &= 0x1f;
                    Some(&buf[0..1])
                } else if ch >= '4' && ch <= '7' {
                    // crossterm returns Control-4 thru 7 for \x1c thru \x1f
                    buf[0] = (buf[0] + 8) & 0x1f;
                    Some(&buf[0..1])
                } else {
                    Some(ch.encode_utf8(&mut buf).as_bytes())
                }
            } else {
                Some(ch.encode_utf8(&mut buf).as_bytes())
            }
        }
        _ => None,
    };
    if let Some(key_str) = key_str {
        Some(Bytes::copy_from_slice(key_str))
    } else {
        None
    }
}

#[tokio::main]
async fn main() -> tokio_serial::Result<()> {
    let mut args = env::args();
    let tty_path = args.nth(1).unwrap_or_else(|| DEFAULT_TTY.into());

    let mut keyboard_stream = EventStream::new();

    let mut port = tokio_serial::new(tty_path, 115_200).open_native_async()?;

    #[cfg(unix)]
    port.set_exclusive(false)
        .expect("Unable to set serial port exclusive to false");

    let stream = LineCodec.framed(port);
    let (mut tx, mut rx) = stream.split();

    loop {
        let keyboard_event = keyboard_stream.next().fuse();
        let event = rx.next().fuse();

        tokio::select! {
            maybe_keyboard_event = keyboard_event => {
                match maybe_keyboard_event {
                    Some(Ok(real_keyboard_event)) => {
                        match real_keyboard_event {
                            Event::Key(key_event) => {

                                if let Some(key_str) = handle_key_press(key_event) {
                                    let test = String::from_utf8_lossy(key_str.as_ref());
                                    // println!("{test}");
                                    let write_result = tx.send(String::from(test)).await;
                                        // sleep(Duration::from_secs(2)).await;
                                        // match write_result {
                                        //     Ok(_) => (),
                                        //     Err(err) => println!("{:?}", err),
                                        // }
                                }

                            },
                            Event::Mouse(mouse_event) => println!("{:?}", mouse_event),
                            Event::Resize(_, _) => todo!(),
                        }
                    },
                    Some(Err(e)) => println!("Error in keyboard stream: {}", e),
                    None => { println!("maybe_keyboard_event return None.")}
                }
            },
            maybe_serial_event = event => {
                match maybe_serial_event {
                    Some(real_serial_event) => match real_serial_event {
                        Ok(chars) => print!("{}", chars),
                        Err(err) =>  println!("{}", err),
                    },
                    None => println!("Uh-oh"),
                }
            },
            // TODO: Add other events, key handling, etc.
        }
    }

    // tokio::spawn(async move {
    //     loop {
    //         let item = rx
    //             .next()
    //             .await
    //             .expect("Error awaiting future in RX stream.")
    //             .expect("Reading stream resulted in an error");
    //         print!("{item}");
    //         sleep(Duration::from_millis(10)).await;
    //     }
    // });

    // tokio::spawn(async move {
    //     loop {
    //         let write_result = tx
    //             .send(String::from(format!("{}\r", r#"print("hello")"#)))
    //             .await;
    //         sleep(Duration::from_secs(2)).await;
    //         match write_result {
    //             Ok(_) => (),
    //             Err(err) => println!("{:?}", err),
    //         }
    //     }
    // });

    // loop {}
    Ok(())
}
