#[cfg(not(any(feature="tokio", feature="async-std", feature="smol", feature="glommio")))]
compile_error! {"One feature flag must be activated"}

#[cfg(any(
    all(feature="tokio", any(feature="async-std",feature="smol",feature="glommio")),
    all(feature="async-std", any(feature="smol",feature="glommio",feature="tokio")),
    all(feature="smol", any(feature="glommio",feature="tokio",feature="async-std")),
    all(feature="glommio", any(feature="tokio", feature="async-std",feature="smol",)),
))]
compile_error! {"More than one feature flags can't be activated"}

mod runtime {
    #[cfg(feature="tokio")]
    pub use {
        tokio::net::TcpStream,
        tokio::io::AsyncReadExt as Read,
        tokio::io::AsyncWriteExt as Write,
        tokio::sync::RwLock
    };

    #[cfg(feature="async-std")]
    pub use {
        async_std::net::TcpStream,
        async_std::io::ReadExt as Read,
        async_std::io::WriteExt as Write,
        async_std::sync::RwLock
    };

    #[cfg(feature="smol")]
    pub use {
        smol::net::TcpStream,
        smol::io::AsyncReadExt as Read,
        smol::io::AsyncWriteExt as Write,
        smol::lock::RwLock
    };

    #[cfg(feature="glommio")]
    pub use {
        glommio::net::TcpStream,
        futures_util::AsyncReadExt as Read,
        futures_util::AsyncWriteExt as Write,
        glommio::sync::RwLock
    };
}

mod connection;
mod handler;
mod frame;
mod message;

pub use connection::{Connection, Closer};
pub use connection::split::{self, ReadHalf, WriteHalf};
pub use handler::Handler;
pub use frame::CloseCode;
pub use message::{Message, CloseFrame};

///////////////////////////////////////////////////////////////////////////

pub(crate) use connection::UnderlyingConnection;

pub struct WebSocket<C: UnderlyingConnection = crate::runtime::TcpStream> {
    /// signed `Sec-WebSocket-Key`
    pub sec_websocket_key: String,
    pub config:            Config,
    pub handler:           Handler<C>,
    _priv: ()
}

/// ## Note
/// 
/// Currently, subprotocols via `Sec-WebSocket-Protocol` is not supported
#[derive(Clone, Debug)]
pub struct Config {
    pub write_buffer_size:      usize,
    pub max_write_buffer_size:  usize,
    pub accept_unmasked_frames: bool,
    pub max_message_size:       Option<usize>,
    pub max_frame_size:         Option<usize>,
}
const _: () = {
    impl Default for Config {
        fn default() -> Self {
            Self {
                write_buffer_size:      128 * 1024, // 128 KiB
                max_write_buffer_size:  usize::MAX,
                accept_unmasked_frames: false,
                max_message_size:       Some(64 << 20),
                max_frame_size:         Some(16 << 20),
            }
        }
    }
};

pub struct WebSocketContext<'ctx> {
    sec_websocket_key: &'ctx str
}
impl<'ctx> WebSocketContext<'ctx> {
    /// create `WebSocketContext` with `Sec-WebSocket-Key` request header value.
    pub fn new(sec_websocket_key: &'ctx str) -> Self {
        Self { sec_websocket_key }
    }

    /// create a WebSocket session with the handler and default config.\
    /// use [`connect_with`](WebSocketContext::connect_with) to apply custom `Config`.
    /// 
    /// ## handler
    /// 
    /// Any `FnOnce + Send + Sync` returning `Send + Future`
    /// with following args and `Output`:
    /// 
    /// * `(Connection) -> () | std::io::Result<()>`
    /// * `(ReadHalf, WriteHalf) -> () | std::io::Result<()>`
    pub fn connect<C: UnderlyingConnection, T>(
        self,
        handler: impl handler::IntoHandler<C, T>
    ) -> WebSocket<C> {
        self.connect_with(Config::default(), handler)
    }

    /// create a WebSocket session with the config the handler.
    /// 
    /// ## handler
    /// 
    /// Any `FnOnce + Send + Sync` returning `Send + Future`
    /// with following args and `Output`:
    /// 
    /// * `(Connection) -> () | std::io::Result<()>`
    /// * `(ReadHalf, WriteHalf) -> () | std::io::Result<()>`
    pub fn connect_with<C: UnderlyingConnection, T>(
        self,
        config: Config,
        handler: impl handler::IntoHandler<C, T>
    ) -> WebSocket<C> {
        WebSocket {
            sec_websocket_key: sign(&self.sec_websocket_key),
            config,
            handler: handler.into_handler(),
            _priv: ()
        }
    }
}

#[inline]
fn sign(sec_websocket_key: &str) -> String {
    use ::sha1::{Sha1, Digest};
    use ::base64::engine::{Engine, general_purpose::STANDARD};

    let mut sha1 = <Sha1 as Digest>::new();
    sha1.update(sec_websocket_key.as_bytes());
    sha1.update(b"258EAFA5-E914-47DA-95CA-C5AB0DC85B11");

    Engine::encode(&STANDARD, sha1.finalize())
}

#[cfg(test)]
#[test] fn test_sign() {
    /* example of https://developer.mozilla.org/en-US/docs/Web/API/WebSockets_API/Writing_WebSocket_servers#server_handshake_response */
    assert_eq!(sign("dGhlIHNhbXBsZSBub25jZQ=="), "s3pPLMBiTxaQ9kYGzzhZRbK+xOo=");
}
