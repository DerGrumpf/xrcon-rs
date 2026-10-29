use pyo3::create_exception;
use pyo3::exceptions::{PyOSError, PyValueError};
use pyo3::prelude::*;

create_exception!(xrcon._xrcon, ParseError, PyValueError);

pub(crate) fn perr(e: xrcon::Error) -> PyErr {
    ParseError::new_err(e.to_string())
}

pub(crate) fn net_err(e: xrcon::net::Error) -> PyErr {
    PyOSError::new_err(e.to_string())
}

pub(crate) fn tokio_err(e: xrcon::tokio::Error) -> PyErr {
    PyOSError::new_err(e.to_string())
}
