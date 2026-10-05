//! Python bindings: a drop-in `chardet` extension module backed by the Rust core.

use pyo3::exceptions::PyValueError;
use pyo3::prelude::*;
use pyo3::types::{PyByteArray, PyBytes, PyDict, PyList};

const VERSION: &str = env!("CARGO_PKG_VERSION");
const DEFAULT_MAX_BYTES: usize = 200_000;
const MINIMUM_THRESHOLD: f64 = 0.20;

fn extract_bytes(obj: &Bound<'_, PyAny>) -> PyResult<Vec<u8>> {
    if let Ok(b) = obj.downcast::<PyBytes>() {
        return Ok(b.as_bytes().to_vec());
    }
    if let Ok(b) = obj.downcast::<PyByteArray>() {
        return Ok(b.to_vec());
    }
    Err(PyValueError::new_err("byte_str must be bytes or bytearray"))
}

/// Emit a Python warning without failing if the warnings module misbehaves.
fn emit_warning(py: Python<'_>, message: &str, category: &str) {
    let Ok(warnings) = py.import("warnings") else {
        return;
    };
    let Ok(builtins) = py.import("builtins") else {
        return;
    };
    let Ok(cat) = builtins.getattr(category) else {
        return;
    };
    let _ = warnings.call_method1("warn", (message, cat, 4usize));
}

fn result_to_dict<'py>(
    py: Python<'py>,
    r: &chardet_core::DetectionResult,
) -> PyResult<Bound<'py, PyDict>> {
    let d = PyDict::new(py);
    match &r.encoding {
        Some(e) => d.set_item("encoding", e)?,
        None => d.set_item("encoding", py.None())?,
    }
    d.set_item("confidence", r.confidence)?;
    match &r.language {
        Some(l) => d.set_item("language", l)?,
        None => d.set_item("language", py.None())?,
    }
    match &r.mime_type {
        Some(m) => d.set_item("mime_type", m)?,
        None => d.set_item("mime_type", py.None())?,
    }
    Ok(d)
}

fn normalize_list(value: Option<Vec<String>>) -> Option<Vec<String>> {
    value
}

#[allow(clippy::too_many_arguments)]
fn make_options(
    encoding_era: i64,
    max_bytes: usize,
    prefer_superset: bool,
    compat_names: bool,
    include_encodings: Option<Vec<String>>,
    exclude_encodings: Option<Vec<String>>,
    no_match_encoding: &str,
    empty_input_encoding: &str,
    ignore_threshold: bool,
) -> PyResult<chardet_core::DetectOptions> {
    Ok(chardet_core::DetectOptions {
        encoding_era: encoding_era as u8,
        max_bytes,
        prefer_superset,
        compat_names,
        include_encodings: normalize_list(include_encodings),
        exclude_encodings: normalize_list(exclude_encodings),
        no_match_encoding: no_match_encoding.to_string(),
        empty_input_encoding: empty_input_encoding.to_string(),
        ignore_threshold,
    })
}

#[pyfunction]
#[pyo3(signature = (
    byte_str,
    should_rename_legacy = false,
    encoding_era = 63i64,
    chunk_size = 65536usize,
    max_bytes = DEFAULT_MAX_BYTES,
    *,
    prefer_superset = false,
    compat_names = true,
    include_encodings = None,
    exclude_encodings = None,
    no_match_encoding = "cp1252",
    empty_input_encoding = "utf-8",
))]
#[allow(clippy::too_many_arguments)]
fn detect<'py>(
    py: Python<'py>,
    byte_str: &Bound<'py, PyAny>,
    should_rename_legacy: bool,
    encoding_era: i64,
    chunk_size: usize,
    max_bytes: usize,
    prefer_superset: bool,
    compat_names: bool,
    include_encodings: Option<Vec<String>>,
    exclude_encodings: Option<Vec<String>>,
    no_match_encoding: &str,
    empty_input_encoding: &str,
) -> PyResult<Bound<'py, PyDict>> {
    if max_bytes < 1 {
        return Err(PyValueError::new_err(
            "max_bytes must be a positive integer",
        ));
    }
    if chunk_size != 65536 {
        emit_warning(
            py,
            "chunk_size is not used in this version of chardet and will be ignored",
            "DeprecationWarning",
        );
    }
    if should_rename_legacy {
        emit_warning(
            py,
            "should_rename_legacy is deprecated, use prefer_superset instead",
            "DeprecationWarning",
        );
    }
    let prefer_superset = prefer_superset || should_rename_legacy;
    let data = extract_bytes(byte_str)?;
    let opts = make_options(
        encoding_era,
        max_bytes,
        prefer_superset,
        compat_names,
        include_encodings,
        exclude_encodings,
        no_match_encoding,
        empty_input_encoding,
        false,
    )?;
    let r = chardet_core::detect_with(&data, &opts).map_err(PyValueError::new_err)?;
    result_to_dict(py, &r)
}

#[pyfunction]
#[pyo3(signature = (
    byte_str,
    ignore_threshold = false,
    should_rename_legacy = false,
    encoding_era = 63i64,
    chunk_size = 65536usize,
    max_bytes = DEFAULT_MAX_BYTES,
    *,
    prefer_superset = false,
    compat_names = true,
    include_encodings = None,
    exclude_encodings = None,
    no_match_encoding = "cp1252",
    empty_input_encoding = "utf-8",
))]
#[allow(clippy::too_many_arguments)]
fn detect_all<'py>(
    py: Python<'py>,
    byte_str: &Bound<'py, PyAny>,
    ignore_threshold: bool,
    should_rename_legacy: bool,
    encoding_era: i64,
    chunk_size: usize,
    max_bytes: usize,
    prefer_superset: bool,
    compat_names: bool,
    include_encodings: Option<Vec<String>>,
    exclude_encodings: Option<Vec<String>>,
    no_match_encoding: &str,
    empty_input_encoding: &str,
) -> PyResult<Bound<'py, PyList>> {
    if max_bytes < 1 {
        return Err(PyValueError::new_err(
            "max_bytes must be a positive integer",
        ));
    }
    if chunk_size != 65536 {
        emit_warning(
            py,
            "chunk_size is not used in this version of chardet and will be ignored",
            "DeprecationWarning",
        );
    }
    if should_rename_legacy {
        emit_warning(
            py,
            "should_rename_legacy is deprecated, use prefer_superset instead",
            "DeprecationWarning",
        );
    }
    let prefer_superset = prefer_superset || should_rename_legacy;
    let data = extract_bytes(byte_str)?;
    let opts = make_options(
        encoding_era,
        max_bytes,
        prefer_superset,
        compat_names,
        include_encodings,
        exclude_encodings,
        no_match_encoding,
        empty_input_encoding,
        ignore_threshold,
    )?;
    let results = chardet_core::detect_all(&data, &opts).map_err(PyValueError::new_err)?;
    let out = PyList::empty(py);
    for r in &results {
        out.append(result_to_dict(py, r)?)?;
    }
    Ok(out)
}

/// Streaming detector, mirroring `chardet.UniversalDetector`.
#[pyclass]
struct UniversalDetector {
    buffer: Vec<u8>,
    opts: chardet_core::DetectOptions,
    done: bool,
    closed: bool,
    result: Option<chardet_core::DetectionResult>,
    input_truncated: bool,
}

#[pymethods]
impl UniversalDetector {
    #[new]
    #[pyo3(signature = (
        lang_filter = 31i64,
        should_rename_legacy = false,
        encoding_era = 63i64,
        max_bytes = DEFAULT_MAX_BYTES,
        *,
        prefer_superset = false,
        compat_names = true,
        include_encodings = None,
        exclude_encodings = None,
        no_match_encoding = "cp1252",
        empty_input_encoding = "utf-8",
    ))]
    #[allow(clippy::too_many_arguments)]
    fn new(
        py: Python<'_>,
        lang_filter: i64,
        should_rename_legacy: bool,
        encoding_era: i64,
        max_bytes: usize,
        prefer_superset: bool,
        compat_names: bool,
        include_encodings: Option<Vec<String>>,
        exclude_encodings: Option<Vec<String>>,
        no_match_encoding: &str,
        empty_input_encoding: &str,
    ) -> PyResult<Self> {
        if max_bytes < 1 {
            return Err(PyValueError::new_err(
                "max_bytes must be a positive integer",
            ));
        }
        if lang_filter != 31 {
            emit_warning(
                py,
                "lang_filter is not implemented in this version of chardet and will be ignored",
                "DeprecationWarning",
            );
        }
        if should_rename_legacy {
            emit_warning(
                py,
                "should_rename_legacy is deprecated, use prefer_superset instead",
                "DeprecationWarning",
            );
        }
        let prefer_superset = prefer_superset || should_rename_legacy;
        let opts = make_options(
            encoding_era,
            max_bytes,
            prefer_superset,
            compat_names,
            include_encodings,
            exclude_encodings,
            no_match_encoding,
            empty_input_encoding,
            false,
        )?;
        Ok(UniversalDetector {
            buffer: Vec::new(),
            opts,
            done: false,
            closed: false,
            result: None,
            input_truncated: false,
        })
    }

    #[getter]
    fn done(&self) -> bool {
        self.done
    }

    fn feed(&mut self, byte_str: &Bound<'_, PyAny>) -> PyResult<()> {
        if self.closed {
            return Err(PyValueError::new_err(
                "feed() called after close() without reset()",
            ));
        }
        if self.done {
            let data = extract_bytes(byte_str)?;
            if !data.is_empty() {
                self.input_truncated = true;
            }
            return Ok(());
        }
        let data = extract_bytes(byte_str)?;
        let remaining = self.opts.max_bytes.saturating_sub(self.buffer.len());
        if remaining > 0 {
            let take = data.len().min(remaining);
            self.buffer.extend_from_slice(&data[..take]);
        }
        if data.len() > remaining {
            self.input_truncated = true;
        }
        if self.buffer.len() >= self.opts.max_bytes {
            self.done = true;
        }
        Ok(())
    }

    fn close<'py>(&mut self, py: Python<'py>) -> PyResult<Bound<'py, PyDict>> {
        if !self.closed {
            self.closed = true;
            let mut run_opts = chardet_core::PipelineOptions {
                encoding_era: self.opts.encoding_era,
                max_bytes: self.opts.max_bytes,
                include_encodings: self.opts.include_encodings.clone(),
                exclude_encodings: self.opts.exclude_encodings.clone(),
                no_match_encoding: self.opts.no_match_encoding.clone(),
                empty_input_encoding: self.opts.empty_input_encoding.clone(),
                full_ranking: false,
                input_truncated: self.input_truncated,
            };
            run_opts.full_ranking = false;
            let results = chardet_core::run_pipeline(&self.buffer, &run_opts);
            let mut first = results
                .into_iter()
                .next()
                .unwrap_or_else(chardet_core::result::none_result);
            if self.opts.prefer_superset {
                chardet_core::output_names::apply_preferred_superset(
                    &mut first,
                    Some(&self.buffer),
                );
            }
            if self.opts.compat_names {
                chardet_core::output_names::apply_compat_names(&mut first);
            }
            self.result = Some(first);
            self.done = true;
        }
        result_to_dict(py, self.result.as_ref().unwrap())
    }

    fn reset(&mut self) {
        self.buffer.clear();
        self.input_truncated = false;
        self.done = false;
        self.closed = false;
        self.result = None;
    }

    #[getter]
    fn result<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyDict>> {
        match &self.result {
            Some(r) => result_to_dict(py, r),
            None => {
                let d = PyDict::new(py);
                d.set_item("encoding", py.None())?;
                d.set_item("confidence", 0.0)?;
                d.set_item("language", py.None())?;
                d.set_item("mime_type", py.None())?;
                Ok(d)
            }
        }
    }

    #[classattr]
    #[pyo3(name = "MINIMUM_THRESHOLD")]
    fn minimum_threshold() -> f64 {
        MINIMUM_THRESHOLD
    }
}

/// Encoding-era flags, mirroring `chardet.enums.EncodingEra`.
#[pyclass]
struct EncodingEra;

#[pymethods]
impl EncodingEra {
    #[classattr]
    #[pyo3(name = "MODERN_WEB")]
    fn modern_web() -> u8 {
        1
    }
    #[classattr]
    #[pyo3(name = "LEGACY_ISO")]
    fn legacy_iso() -> u8 {
        2
    }
    #[classattr]
    #[pyo3(name = "LEGACY_MAC")]
    fn legacy_mac() -> u8 {
        4
    }
    #[classattr]
    #[pyo3(name = "LEGACY_REGIONAL")]
    fn legacy_regional() -> u8 {
        8
    }
    #[classattr]
    #[pyo3(name = "DOS")]
    fn dos() -> u8 {
        16
    }
    #[classattr]
    #[pyo3(name = "MAINFRAME")]
    fn mainframe() -> u8 {
        32
    }
    #[classattr]
    #[pyo3(name = "ALL")]
    fn all() -> u8 {
        63
    }
}

#[pymodule]
fn chardet(m: &Bound<'_, PyModule>) -> PyResult<()> {
    fill_module(m)
}

/// Native submodule for the `chardet` Python package (`chardet._chardet`).
#[pymodule]
fn _chardet(m: &Bound<'_, PyModule>) -> PyResult<()> {
    fill_module(m)
}

/// Alias module used by tests to load the extension alongside Python chardet.
#[pymodule]
fn chardet_rs(m: &Bound<'_, PyModule>) -> PyResult<()> {
    fill_module(m)
}

fn fill_module(m: &Bound<'_, PyModule>) -> PyResult<()> {
    m.add("__version__", VERSION)?;
    m.add("DEFAULT_MAX_BYTES", DEFAULT_MAX_BYTES)?;
    m.add("MINIMUM_THRESHOLD", MINIMUM_THRESHOLD)?;
    m.add_function(wrap_pyfunction!(detect, m)?)?;
    m.add_function(wrap_pyfunction!(detect_all, m)?)?;
    m.add_class::<UniversalDetector>()?;
    m.add_class::<EncodingEra>()?;

    // UniversalDetector.LEGACY_MAP (chardet 6.x compatibility).
    let py = m.py();
    let legacy_map = PyDict::new(py);
    for (k, v) in [
        ("ascii", "cp1252"),
        ("euc_kr", "cp949"),
        ("iso8859-1", "cp1252"),
        ("iso8859-2", "cp1250"),
        ("iso8859-5", "cp1251"),
        ("iso8859-6", "cp1256"),
        ("iso8859-7", "cp1253"),
        ("iso8859-8", "cp1255"),
        ("iso8859-9", "cp1254"),
        ("iso8859-11", "cp874"),
        ("iso8859-13", "cp1257"),
        ("tis-620", "cp874"),
    ] {
        legacy_map.set_item(k, v)?;
    }
    let cls = m.getattr("UniversalDetector")?;
    cls.setattr("LEGACY_MAP", legacy_map)?;
    Ok(())
}
