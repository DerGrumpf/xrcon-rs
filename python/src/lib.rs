use std::collections::HashMap;

use pyo3::IntoPyObjectExt;
use pyo3::create_exception;
use pyo3::exceptions::PyValueError;
use pyo3::prelude::*;
use pyo3::types::{PyBytes, PyDict, PyList, PyTuple};
use xrcon::core::{Event, Mode, PingKind, Session};
use xrcon::net::Client as NetClient;
use xrcon::parse::{self, Player};

use std::borrow::Cow;
use std::sync::Arc;
use tokio::sync::Mutex;
use xrcon_tokio::Client as TokioClient;

create_exception!(xrcon._xrcon, ParseError, PyValueError);

fn perr(e: xrcon::Error) -> PyErr {
    ParseError::new_err(e.to_string())
}

fn vars_dict<'py>(
    py: Python<'py>,
    vars: &HashMap<Vec<u8>, Vec<u8>>,
) -> PyResult<Bound<'py, PyDict>> {
    let dict = PyDict::new(py);
    for (k, v) in vars {
        dict.set_item(PyBytes::new(py, k), PyBytes::new(py, v))?;
    }
    Ok(dict)
}

fn players_list<'py>(py: Python<'py>, players: &[Player]) -> PyResult<Bound<'py, PyList>> {
    PyList::new(
        py,
        players
            .iter()
            .map(|p| (p.frags, p.ping, PyBytes::new(py, &p.name))),
    )
}

fn mode_from(value: u8) -> PyResult<Mode> {
    match value {
        0 => Ok(Mode::Nosecure),
        1 => Ok(Mode::Time),
        2 => Ok(Mode::Challenge),
        _ => Err(PyValueError::new_err("mode must be 0, 1 or 2")),
    }
}

fn kind_from(value: &str) -> PyResult<PingKind> {
    match value {
        "q2" => Ok(PingKind::Q2),
        "q3" => Ok(PingKind::Q3),
        _ => Err(PyValueError::new_err("kind must be 'q2' or 'q3'")),
    }
}

fn kind_name(kind: PingKind) -> &'static str {
    match kind {
        PingKind::Q2 => "q2",
        PingKind::Q3 => "q3",
    }
}

/// Turns an event into a tuple whose first item names the event.
fn event_to_py<'py>(py: Python<'py>, event: Event) -> PyResult<Bound<'py, PyTuple>> {
    let items: Vec<Bound<'py, PyAny>> = match event {
        Event::Response(body) => vec![
            "response".into_bound_py_any(py)?,
            PyBytes::new(py, &body).into_any(),
        ],
        Event::ChallengeFailed => vec!["challenge_failed".into_bound_py_any(py)?],
        Event::Pong { kind, rtt } => vec![
            "pong".into_bound_py_any(py)?,
            kind_name(kind).into_bound_py_any(py)?,
            rtt.into_bound_py_any(py)?,
        ],
        Event::PingTimeout(kind) => vec![
            "ping_timeout".into_bound_py_any(py)?,
            kind_name(kind).into_bound_py_any(py)?,
        ],
        Event::Status { vars, players } => vec![
            "status".into_bound_py_any(py)?,
            vars_dict(py, &vars)?.into_any(),
            players_list(py, &players)?.into_any(),
        ],
        Event::StatusInvalid => vec!["status_invalid".into_bound_py_any(py)?],
        Event::StatusTimeout => vec!["status_timeout".into_bound_py_any(py)?],
    };
    PyTuple::new(py, items)
}

/// Sans-IO rcon session. Mode: 0 = nosecure, 1 = time, 2 = challenge.
#[pyclass(name = "Session")]
struct PySession {
    inner: Session,
}

#[pymethods]
impl PySession {
    #[new]
    #[pyo3(signature = (password, mode = 1))]
    fn new(password: &[u8], mode: u8) -> PyResult<Self> {
        Ok(Self {
            inner: Session::new(password, mode_from(mode)?),
        })
    }

    fn set_mode(&mut self, mode: u8) -> PyResult<()> {
        self.inner.set_mode(mode_from(mode)?);
        Ok(())
    }

    fn set_password(&mut self, password: &[u8]) {
        self.inner.set_password(password);
    }

    fn set_time_diff(&mut self, seconds: i64) {
        self.inner.set_time_diff(seconds);
    }

    fn set_challenge_timeout(&mut self, seconds: f64) {
        self.inner.set_challenge_timeout(seconds);
    }

    fn set_challenge_retries(&mut self, tries: u32) {
        self.inner.set_challenge_retries(tries);
    }

    fn send(&mut self, command: &[u8], now: f64) {
        self.inner.send(command, now);
    }

    /// `kind` is 'q2' or 'q3'.
    fn ping(&mut self, kind: &str, timeout: f64, now: f64) -> PyResult<()> {
        self.inner.ping(kind_from(kind)?, timeout, now);
        Ok(())
    }

    fn getstatus(&mut self, timeout: f64, now: f64) {
        self.inner.getstatus(timeout, now);
    }

    fn handle_datagram(&mut self, datagram: &[u8], now: f64) {
        self.inner.handle_datagram(datagram, now);
    }

    fn handle_timeout(&mut self, now: f64) {
        self.inner.handle_timeout(now);
    }

    fn poll_transmit<'py>(&mut self, py: Python<'py>) -> Option<Bound<'py, PyBytes>> {
        self.inner.poll_transmit().map(|p| PyBytes::new(py, &p))
    }

    fn poll_event<'py>(&mut self, py: Python<'py>) -> PyResult<Option<Bound<'py, PyTuple>>> {
        self.inner
            .poll_event()
            .map(|event| event_to_py(py, event))
            .transpose()
    }

    fn poll_timeout(&self) -> Option<f64> {
        self.inner.poll_timeout()
    }
}

/// Converts Xonotic color codes to ANSI escapes.
#[pyfunction]
#[pyo3(name = "colorize")]
fn py_colorize<'py>(py: Python<'py>, data: &[u8]) -> Bound<'py, PyBytes> {
    PyBytes::new(py, &xrcon::color::colorize(data))
}

#[pyfunction]
#[pyo3(name = "parse_challenge_response")]
fn py_parse_challenge_response<'py>(
    py: Python<'py>,
    packet: &[u8],
) -> PyResult<Bound<'py, PyBytes>> {
    let c = parse::parse_challenge_response(packet).map_err(perr)?;
    Ok(PyBytes::new(py, c))
}

#[pyfunction]
#[pyo3(name = "parse_rcon_response")]
fn py_parse_rcon_response<'py>(py: Python<'py>, packet: &[u8]) -> PyResult<Bound<'py, PyBytes>> {
    let b = parse::parse_rcon_response(packet).map_err(perr)?;
    Ok(PyBytes::new(py, b))
}

#[pyfunction]
#[pyo3(name = "parse_server_addr", signature = (text, default_port = 26000))]
fn py_parse_server_addr(text: &str, default_port: u16) -> PyResult<(String, u16)> {
    parse::parse_server_addr(text, default_port).map_err(perr)
}

#[pyfunction]
#[pyo3(name = "parse_server_vars")]
fn py_parse_server_vars<'py>(py: Python<'py>, vars: &[u8]) -> PyResult<Bound<'py, PyDict>> {
    let map = parse::parse_server_vars(vars).map_err(perr)?;
    vars_dict(py, &map)
}

/// Returns (vars, players); players are plain tuples here.
#[pyfunction]
#[pyo3(name = "parse_status_packet")]
fn py_parse_status_packet<'py>(
    py: Python<'py>,
    packet: &[u8],
) -> PyResult<(Bound<'py, PyDict>, Bound<'py, PyList>)> {
    let (vars, players) = parse::parse_status_packet(packet).map_err(perr)?;
    Ok((vars_dict(py, &vars)?, players_list(py, &players)?))
}

/// Returns a list of (ip as str, port).
#[pyfunction]
#[pyo3(name = "parse_servers_response")]
fn py_parse_servers_response(packet: &[u8]) -> PyResult<Vec<(String, u16)>> {
    let servers = parse::parse_servers_response(packet).map_err(perr)?;
    Ok(servers
        .into_iter()
        .map(|(ip, port)| (ip.to_string(), port))
        .collect())
}

fn net_err(e: xrcon::net::Error) -> PyErr {
    pyo3::exceptions::PyOSError::new_err(e.to_string())
}

/// Blocking rcon client.
#[pyclass(name = "Client")]
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

    #[pyo3(signature = (command, timeout = 0.7))]
    fn execute<'py>(
        &mut self,
        py: Python<'py>,
        command: &[u8],
        timeout: f64,
    ) -> PyResult<Bound<'py, PyBytes>> {
        let out = py
            .detach(|| self.inner.execute(command, timeout))
            .map_err(net_err)?;
        Ok(PyBytes::new(py, &out))
    }

    #[pyo3(signature = (kind = "q2", timeout = 1.0))]
    fn ping(&mut self, py: Python<'_>, kind: &str, timeout: f64) -> PyResult<Option<f64>> {
        let kind = kind_from(kind)?;
        py.detach(|| self.inner.ping(kind, timeout))
            .map_err(net_err)
    }

    #[pyo3(signature = (timeout = 1.0))]
    fn getstatus<'py>(
        &mut self,
        py: Python<'py>,
        timeout: f64,
    ) -> PyResult<Option<(Bound<'py, PyDict>, Bound<'py, PyList>)>> {
        match py
            .detach(|| self.inner.getstatus(timeout))
            .map_err(net_err)?
        {
            Some((vars, players)) => Ok(Some((vars_dict(py, &vars)?, players_list(py, &players)?))),
            None => Ok(None),
        }
    }
}

fn tokio_err(e: xrcon_tokio::Error) -> PyErr {
    pyo3::exceptions::PyOSError::new_err(e.to_string())
}

/// Async rcon client (Tokio).
#[pyclass(name = "AsyncClient")]
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

    fn close<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyAny>> {
        let inner = self.inner.clone();
        pyo3_async_runtimes::tokio::future_into_py(py, async move {
            *inner.lock().await = None;
            Ok(())
        })
    }

    #[pyo3(signature = (command, timeout = 0.7))]
    fn execute<'py>(
        &self,
        py: Python<'py>,
        command: &[u8],
        timeout: f64,
    ) -> PyResult<Bound<'py, PyAny>> {
        let (inner, command) = (self.inner.clone(), command.to_vec());
        pyo3_async_runtimes::tokio::future_into_py(py, async move {
            let mut g = inner.lock().await;
            let c = g
                .as_mut()
                .ok_or_else(|| PyValueError::new_err("not connected"))?;
            let out = c.execute(&command, timeout).await.map_err(tokio_err)?;
            Ok(Cow::<'static, [u8]>::Owned(out))
        })
    }

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

    #[pyo3(signature = (timeout = 1.0))]
    fn getstatus<'py>(&self, py: Python<'py>, timeout: f64) -> PyResult<Bound<'py, PyAny>> {
        let inner = self.inner.clone();
        pyo3_async_runtimes::tokio::future_into_py(py, async move {
            let mut g = inner.lock().await;
            let c = g
                .as_mut()
                .ok_or_else(|| PyValueError::new_err("not connected"))?;
            let status = c.getstatus(timeout).await.map_err(tokio_err)?;
            Python::attach(|py| -> PyResult<Option<Py<PyAny>>> {
                match status {
                    Some((vars, players)) => Ok(Some(
                        PyTuple::new(
                            py,
                            [
                                vars_dict(py, &vars)?.into_any(),
                                players_list(py, &players)?.into_any(),
                            ],
                        )?
                        .into_any()
                        .unbind(),
                    )),
                    None => Ok(None),
                }
            })
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
    m.add_function(wrap_pyfunction!(py_parse_status_packet, m)?)?;
    m.add_function(wrap_pyfunction!(py_parse_servers_response, m)?)?;
    m.add_class::<PyClient>()?;
    m.add_class::<PyAsyncClient>()?;
    Ok(())
}
