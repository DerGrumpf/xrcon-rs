use pyo3::exceptions::PyValueError;
use pyo3::prelude::*;
use pyo3::types::{PyBytes, PyDict};
use xrcon::core::Mode;
use xrcon::net::Client as NetClient;
use xrcon::parse;

use std::borrow::Cow;
use std::sync::Arc;
use tokio::sync::Mutex;
use xrcon::tokio::Client as TokioClient;

mod errors;
use errors::{ParseError, net_err, perr, tokio_err};

mod convert;
use convert::{kind_from, mode_from, vars_dict};

mod session;
use session::PySession;

/// Converts Xonotic colour codes (`^7`, `^xRGB`) in `data` to ANSI escapes.
///
/// Args:
///     data: Text as bytes.
///
/// Returns:
///     The converted text as bytes.
#[pyfunction]
#[pyo3(name = "colorize")]
fn py_colorize<'py>(py: Python<'py>, data: &[u8]) -> Bound<'py, PyBytes> {
    PyBytes::new(py, &xrcon::color::colorize(data))
}

/// Extracts the challenge from a `getchallenge` reply packet.
///
/// Args:
///     packet: The full reply datagram, as bytes.
///
/// Returns:
///     The challenge as bytes.
///
/// Raises:
///     ParseError: The packet is not a valid challenge reply.
#[pyfunction]
#[pyo3(name = "parse_challenge_response")]
fn py_parse_challenge_response<'py>(
    py: Python<'py>,
    packet: &[u8],
) -> PyResult<Bound<'py, PyBytes>> {
    let c = parse::parse_challenge_response(packet).map_err(perr)?;
    Ok(PyBytes::new(py, c))
}

/// Extracts the reply text from an rcon reply packet.
///
/// Args:
///     packet: The full reply datagram, as bytes.
///
/// Returns:
///     The reply text as bytes, without the packet header.
///
/// Raises:
///     ParseError: The packet is not an rcon reply.
#[pyfunction]
#[pyo3(name = "parse_rcon_response")]
fn py_parse_rcon_response<'py>(py: Python<'py>, packet: &[u8]) -> PyResult<Bound<'py, PyBytes>> {
    let b = parse::parse_rcon_response(packet).map_err(perr)?;
    Ok(PyBytes::new(py, b))
}

/// Splits `host:port` text into a host and a port.
///
/// IPv6 addresses must be in brackets, e.g. `[::1]:26000`.
///
/// Args:
///     text: The address text.
///     default_port: Port used when the text has none (default 26000).
///
/// Returns:
///     A tuple `(host, port)`.
///
/// Raises:
///     ParseError: The address is malformed.
#[pyfunction]
#[pyo3(name = "parse_server_addr", signature = (text, default_port = 26000))]
fn py_parse_server_addr(text: &str, default_port: u16) -> PyResult<(String, u16)> {
    parse::parse_server_addr(text, default_port).map_err(perr)
}

/// Parses a server variables string (`\key\value\key\value`).
///
/// Args:
///     vars: The variables line, as bytes.
///
/// Returns:
///     A dict mapping keys to values, both bytes.
///
/// Raises:
///     ParseError: The string is malformed.
#[pyfunction]
#[pyo3(name = "parse_server_vars")]
fn py_parse_server_vars<'py>(py: Python<'py>, vars: &[u8]) -> PyResult<Bound<'py, PyDict>> {
    let map = parse::parse_server_vars(vars).map_err(perr)?;
    vars_dict(py, &map)
}

/// Parses a master-server reply into a list of servers.
///
/// Args:
///     packet: The full reply datagram, as bytes.
///
/// Returns:
///     A list of `(ip, port)` tuples, with the ip as str.
///
/// Raises:
///     ParseError: The packet is malformed or has no terminator.
#[pyfunction]
#[pyo3(name = "parse_servers_response")]
fn py_parse_servers_response(packet: &[u8]) -> PyResult<Vec<(String, u16)>> {
    let servers = parse::parse_servers_response(packet).map_err(perr)?;
    Ok(servers
        .into_iter()
        .map(|(ip, port)| (ip.to_string(), port))
        .collect())
}

/// Blocking rcon client.
///
/// Opens a UDP socket to the server. Each call waits until it is done.
/// The GIL is released while waiting.
///
/// Args:
///     host: Server host name or IP address.
///     password: The rcon password, as bytes.
///     mode: 0 = non-secure, 1 = time-secure (default), 2 = challenge-secure.
///     port: Server port (default 26000).
///
/// Raises:
///     ValueError: `mode` is not 0, 1 or 2.
///     OSError: The host does not resolve or the socket fails.
#[pyclass(name = "Client", module = "xrcon")]
struct PyClient {
    inner: NetClient,
}

#[pymethods]
impl PyClient {
    #[new]
    #[pyo3(signature = (host, password, mode = 1, port = 26000))]
    fn new(host: &str, password: &[u8], mode: u8, port: u16) -> PyResult<Self> {
        let inner = NetClient::connect(host, port, password, mode_from(mode)?).map_err(net_err)?;
        Ok(Self { inner })
    }

    /// Sends an rcon command and returns the reply.
    ///
    /// Collects reply packets until `timeout` seconds after the command was
    /// sent (after the challenge, in challenge mode).
    ///
    /// Args:
    ///     command: The command, as bytes, e.g. `b"status"`.
    ///     timeout: Seconds to collect replies (default 0.7).
    ///     color: If true, convert Xonotic colour codes to ANSI escapes.
    ///
    /// Returns:
    ///     The reply as bytes.
    #[pyo3(signature = (command, timeout = 0.7, color = false))]
    fn execute<'py>(
        &mut self,
        py: Python<'py>,
        command: &[u8],
        timeout: f64,
        color: bool,
    ) -> PyResult<Bound<'py, PyBytes>> {
        let out = py
            .detach(|| self.inner.execute(command, timeout))
            .map_err(net_err)?;
        let out = if color {
            xrcon::color::colorize(&out)
        } else {
            out
        };
        Ok(PyBytes::new(py, &out))
    }

    /// Pings the server.
    ///
    /// Args:
    ///     kind: "q2" (default) or "q3".
    ///     timeout: Seconds to wait for the answer (default 1.0).
    ///
    /// Returns:
    ///     The round trip time in seconds, or None on timeout.
    #[pyo3(signature = (kind = "q2", timeout = 1.0))]
    fn ping(&mut self, py: Python<'_>, kind: &str, timeout: f64) -> PyResult<Option<f64>> {
        let kind = kind_from(kind)?;
        py.detach(|| self.inner.ping(kind, timeout))
            .map_err(net_err)
    }
}

/// Async rcon client (Tokio).
///
/// Use it with `async with`, which connects on entry and closes on exit,
/// or call `connect()` and `close()` yourself. The constructor does not
/// open a socket. Calls on one client run one at a time.
///
/// Args:
///     host: Server host name or IP address.
///     password: The rcon password, as bytes.
///     mode: 0 = non-secure, 1 = time-secure (default), 2 = challenge-secure.
///     port: Server port (default 26000).
///
/// Raises:
///     ValueError: `mode` is not 0, 1 or 2.
#[pyclass(name = "AsyncClient", module = "xrcon")]
struct PyAsyncClient {
    host: String,
    port: u16,
    password: Vec<u8>,
    mode: Mode,
    inner: Arc<Mutex<Option<TokioClient>>>,
}

#[pymethods]
impl PyAsyncClient {
    #[new]
    #[pyo3(signature = (host, password, mode = 1, port = 26000))]
    fn new(host: String, password: &[u8], mode: u8, port: u16) -> PyResult<Self> {
        Ok(Self {
            host,
            port,
            password: password.to_vec(),
            mode: mode_from(mode)?,
            inner: Arc::new(Mutex::new(None)),
        })
    }

    /// Connects when entering `async with` and returns the client.
    fn __aenter__<'py>(slf: Py<Self>, py: Python<'py>) -> PyResult<Bound<'py, PyAny>> {
        let inner = slf.borrow(py).inner.clone();
        let (host, port, pw, mode) = {
            let s = slf.borrow(py);
            (s.host.clone(), s.port, s.password.clone(), s.mode)
        };
        pyo3_async_runtimes::tokio::future_into_py(py, async move {
            let c = TokioClient::connect(&host, port, pw, mode)
                .await
                .map_err(tokio_err)?;
            *inner.lock().await = Some(c);
            Ok(slf)
        })
    }

    /// Closes the client when leaving `async with`.
    fn __aexit__<'py>(
        &self,
        py: Python<'py>,
        _exc_type: Option<Bound<'py, PyAny>>,
        _exc: Option<Bound<'py, PyAny>>,
        _tb: Option<Bound<'py, PyAny>>,
    ) -> PyResult<Bound<'py, PyAny>> {
        let inner = self.inner.clone();
        pyo3_async_runtimes::tokio::future_into_py(py, async move {
            *inner.lock().await = None;
            Ok(())
        })
    }

    /// Opens the UDP socket. Awaitable.
    ///
    /// Not needed with `async with`. Must be awaited before `execute` or
    /// `ping` otherwise.
    fn connect<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyAny>> {
        let (host, port, pw, mode) = (
            self.host.clone(),
            self.port,
            self.password.clone(),
            self.mode,
        );
        let inner = self.inner.clone();
        pyo3_async_runtimes::tokio::future_into_py(py, async move {
            let c = TokioClient::connect(&host, port, pw, mode)
                .await
                .map_err(tokio_err)?;
            *inner.lock().await = Some(c);
            Ok(())
        })
    }

    /// Closes the socket. Awaitable.
    fn close<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyAny>> {
        let inner = self.inner.clone();
        pyo3_async_runtimes::tokio::future_into_py(py, async move {
            *inner.lock().await = None;
            Ok(())
        })
    }

    /// Sends an rcon command and returns the reply. Awaitable.
    ///
    /// Collects reply packets until `timeout` seconds after the command was
    /// sent (after the challenge, in challenge mode).
    ///
    /// Args:
    ///     command: The command, as bytes, e.g. `b"status"`.
    ///     timeout: Seconds to collect replies (default 0.7).
    ///     color: If true, convert Xonotic colour codes to ANSI escapes.
    ///
    /// Returns:
    ///     The reply as bytes.
    ///
    /// Raises:
    ///     ValueError: The client is not connected.
    #[pyo3(signature = (command, timeout = 0.7, color = false))]
    fn execute<'py>(
        &self,
        py: Python<'py>,
        command: &[u8],
        timeout: f64,
        color: bool,
    ) -> PyResult<Bound<'py, PyAny>> {
        let (inner, command) = (self.inner.clone(), command.to_vec());
        pyo3_async_runtimes::tokio::future_into_py(py, async move {
            let mut g = inner.lock().await;
            let c = g
                .as_mut()
                .ok_or_else(|| PyValueError::new_err("not connected"))?;
            let out = c.execute(&command, timeout).await.map_err(tokio_err)?;
            let out = if color {
                xrcon::color::colorize(&out)
            } else {
                out
            };
            Ok(Cow::<'static, [u8]>::Owned(out))
        })
    }

    /// Pings the server. Awaitable.
    ///
    /// Args:
    ///     kind: "q2" (default) or "q3".
    ///     timeout: Seconds to wait for the answer (default 1.0).
    ///
    /// Returns:
    ///     The round trip time in seconds, or None on timeout.
    ///
    /// Raises:
    ///     ValueError: The client is not connected, or `kind` is invalid.
    #[pyo3(signature = (kind = "q2", timeout = 1.0))]
    fn ping<'py>(&self, py: Python<'py>, kind: &str, timeout: f64) -> PyResult<Bound<'py, PyAny>> {
        let (inner, kind) = (self.inner.clone(), kind_from(kind)?);
        pyo3_async_runtimes::tokio::future_into_py(py, async move {
            let mut g = inner.lock().await;
            let c = g
                .as_mut()
                .ok_or_else(|| PyValueError::new_err("not connected"))?;
            c.ping(kind, timeout).await.map_err(tokio_err)
        })
    }
}

#[pymodule]
fn _xrcon(m: &Bound<'_, PyModule>) -> PyResult<()> {
    m.add("__version__", env!("CARGO_PKG_VERSION"))?;
    m.add("ParseError", m.py().get_type::<ParseError>())?;
    m.add_class::<PySession>()?;
    m.add_function(wrap_pyfunction!(py_colorize, m)?)?;
    m.add_function(wrap_pyfunction!(py_parse_challenge_response, m)?)?;
    m.add_function(wrap_pyfunction!(py_parse_rcon_response, m)?)?;
    m.add_function(wrap_pyfunction!(py_parse_server_addr, m)?)?;
    m.add_function(wrap_pyfunction!(py_parse_server_vars, m)?)?;
    m.add_function(wrap_pyfunction!(py_parse_servers_response, m)?)?;
    m.add_class::<PyClient>()?;
    m.add_class::<PyAsyncClient>()?;
    Ok(())
}
