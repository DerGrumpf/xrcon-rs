use pyo3::IntoPyObjectExt;
use pyo3::prelude::*;
use pyo3::types::{PyBytes, PyTuple};
use xrcon::core::{Event, Session};

use crate::convert::{kind_from, kind_name, mode_from};

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
    };
    PyTuple::new(py, items)
}

/// Sans-IO rcon session.
///
/// Does no network I/O and has no clock. You send the datagrams it queues
/// over your own UDP socket and pass the current time into each call.
/// One operation (challenge handshake or ping) is pending at a time; a new
/// one replaces the old one without an event.
///
/// Events from `poll_event()` are tuples whose first item is the name:
/// `("response", bytes)`, `("challenge_failed",)`,
/// `("pong", kind, rtt)` and `("ping_timeout", kind)`.
///
/// Args:
///     password: The rcon password, as bytes.
///     mode: 0 = non-secure, 1 = time-secure (default), 2 = challenge-secure.
///
/// Raises:
///     ValueError: `mode` is not 0, 1 or 2.
#[pyclass(name = "Session", module = "xrcon")]
pub(crate) struct PySession {
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

    /// Changes the mode (0, 1 or 2) for following commands.
    fn set_mode(&mut self, mode: u8) -> PyResult<()> {
        self.inner.set_mode(mode_from(mode)?);
        Ok(())
    }

    /// Changes the rcon password for following commands.
    fn set_password(&mut self, password: &[u8]) {
        self.inner.set_password(password);
    }

    /// Sets a time difference in seconds, added to the time sent in
    /// time-secure mode.
    fn set_time_diff(&mut self, seconds: i64) {
        self.inner.set_time_diff(seconds);
    }

    /// Sets the seconds to wait for a challenge reply before retrying.
    fn set_challenge_timeout(&mut self, seconds: f64) {
        self.inner.set_challenge_timeout(seconds);
    }

    /// Sets how many times the challenge request is sent in total.
    fn set_challenge_retries(&mut self, tries: u32) {
        self.inner.set_challenge_retries(tries);
    }

    /// Queues an rcon command. `now` is the current time in seconds.
    ///
    /// In challenge mode this replaces any pending operation.
    fn send(&mut self, command: &[u8], now: f64) {
        self.inner.send(command, now);
    }

    /// Queues a ping. `kind` is 'q2' or 'q3'; the deadline is
    /// `now + timeout`. Replaces any pending operation.
    fn ping(&mut self, kind: &str, timeout: f64, now: f64) -> PyResult<()> {
        self.inner.ping(kind_from(kind)?, timeout, now);
        Ok(())
    }

    /// Feeds one received datagram into the session.
    fn handle_datagram(&mut self, datagram: &[u8], now: f64) {
        self.inner.handle_datagram(datagram, now);
    }

    /// Tells the session the current time. Expires the pending operation
    /// if its deadline has passed.
    fn handle_timeout(&mut self, now: f64) {
        self.inner.handle_timeout(now);
    }

    /// Returns the next datagram to send as bytes, or None.
    fn poll_transmit<'py>(&mut self, py: Python<'py>) -> Option<Bound<'py, PyBytes>> {
        self.inner.poll_transmit().map(|p| PyBytes::new(py, &p))
    }

    /// Returns the next event as a tuple, or None.
    fn poll_event<'py>(&mut self, py: Python<'py>) -> PyResult<Option<Bound<'py, PyTuple>>> {
        self.inner
            .poll_event()
            .map(|event| event_to_py(py, event))
            .transpose()
    }

    /// Returns the deadline of the pending operation (same clock as `now`),
    /// or None if nothing is pending.
    fn poll_timeout(&self) -> Option<f64> {
        self.inner.poll_timeout()
    }
}
