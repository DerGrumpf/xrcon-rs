use std::collections::HashMap;

use pyo3::exceptions::PyValueError;
use pyo3::prelude::*;
use pyo3::types::{PyBytes, PyDict};
use xrcon::core::{Mode, PingKind};

pub(crate) fn mode_from(value: u8) -> PyResult<Mode> {
    match value {
        0 => Ok(Mode::Nosecure),
        1 => Ok(Mode::Time),
        2 => Ok(Mode::Challenge),
        _ => Err(PyValueError::new_err("mode must be 0, 1 or 2")),
    }
}

pub(crate) fn kind_from(value: &str) -> PyResult<PingKind> {
    match value {
        "q2" => Ok(PingKind::Q2),
        "q3" => Ok(PingKind::Q3),
        _ => Err(PyValueError::new_err("kind must be 'q2' or 'q3'")),
    }
}

pub(crate) fn kind_name(kind: PingKind) -> &'static str {
    match kind {
        PingKind::Q2 => "q2",
        PingKind::Q3 => "q3",
    }
}

pub(crate) fn vars_dict<'py>(
    py: Python<'py>,
    vars: &HashMap<Vec<u8>, Vec<u8>>,
) -> PyResult<Bound<'py, PyDict>> {
    let dict = PyDict::new(py);
    for (k, v) in vars {
        dict.set_item(PyBytes::new(py, k), PyBytes::new(py, v))?;
    }
    Ok(dict)
}
