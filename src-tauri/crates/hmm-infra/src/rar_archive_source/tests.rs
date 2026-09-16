use super::*;

#[derive(Default)]
struct RecordingSink {
    bytes: Vec<u8>,
    chunk_sizes: Vec<usize>,
    failure: Option<&'static str>,
    panic_on_write: bool,
}

impl ArchiveChunkSink for RecordingSink {
    fn write_chunk(&mut self, chunk: &[u8]) -> Result<()> {
        assert!(!self.panic_on_write, "synthetic sink panic");
        if let Some(message) = self.failure {
            anyhow::bail!(message);
        }
        self.bytes.extend_from_slice(chunk);
        self.chunk_sizes.push(chunk.len());
        Ok(())
    }
}

fn callback_state(sink: &mut RecordingSink) -> CallbackState {
    CallbackState {
        sink: Some(sink as *mut dyn ArchiveChunkSink),
        abort: None,
    }
}

fn send_chunk(state: &mut CallbackState, chunk: &[u8]) -> c_int {
    // SAFETY: state, its sink and this slice remain alive throughout the synchronous call.
    unsafe {
        rar_callback(
            unrar::UCM_PROCESSDATA,
            state as *mut _ as unrar::Lparam,
            chunk.as_ptr() as unrar::Lparam,
            chunk.len() as unrar::Lparam,
        )
    }
}

#[test]
fn empty_data_callbacks_preserve_the_surrounding_file_bytes() {
    let mut sink = RecordingSink::default();
    let mut state = callback_state(&mut sink);

    assert_eq!(send_chunk(&mut state, b"before"), 1);
    assert_eq!(send_chunk(&mut state, b""), 1);
    // A zero-length block does not require a data pointer; it must not form a null slice.
    let result = unsafe {
        rar_callback(
            unrar::UCM_PROCESSDATA,
            &mut state as *mut _ as unrar::Lparam,
            0,
            0,
        )
    };
    assert_eq!(result, 1);
    assert_eq!(send_chunk(&mut state, b"after"), 1);
    assert_eq!(sink.bytes, b"beforeafter");
    assert_eq!(sink.chunk_sizes, [6, 0, 0, 5]);
    assert!(state.abort.is_none());
}

#[test]
fn empty_data_callbacks_still_observe_sink_cancellation_and_failures() {
    for message in ["mod import cancelled", "synthetic write failure"] {
        let mut sink = RecordingSink {
            failure: Some(message),
            ..Default::default()
        };
        let mut state = callback_state(&mut sink);
        assert_eq!(send_chunk(&mut state, b""), -1);
        assert_eq!(
            state.abort.take().expect("abort reason").to_string(),
            message
        );
        assert!(sink.bytes.is_empty());
    }
}

#[test]
fn nonempty_data_callbacks_preserve_the_sink_failure_reason() {
    let mut sink = RecordingSink {
        failure: Some("unsafe archive: archive file size limit exceeded"),
        ..Default::default()
    };
    let mut state = callback_state(&mut sink);
    assert_eq!(send_chunk(&mut state, b"payload"), -1);
    assert_eq!(
        state.abort.take().expect("abort reason").to_string(),
        "unsafe archive: archive file size limit exceeded"
    );
    assert!(sink.bytes.is_empty());
}

#[test]
fn invalid_callback_arguments_do_not_reach_the_sink() {
    let mut sink = RecordingSink::default();
    let mut state = callback_state(&mut sink);
    let state_ptr = &mut state as *mut _ as unrar::Lparam;
    let data = b"x";
    let data_ptr = data.as_ptr() as unrar::Lparam;
    for (msg, user_data, buffer, length) in [
        (unrar::UCM_PROCESSDATA, 0, data_ptr, 1),
        (unrar::UCM_PROCESSDATA, state_ptr, 0, 1),
        (unrar::UCM_PROCESSDATA, state_ptr, data_ptr, -1),
        (unrar::UCM_PROCESSDATA + 1, state_ptr, data_ptr, 1),
    ] {
        // SAFETY: non-null pointers refer to live objects; invalid shapes must be rejected
        // before a data slice is formed or a missing user_data pointer is dereferenced.
        assert_eq!(unsafe { rar_callback(msg, user_data, buffer, length) }, -1);
    }
    assert!(sink.chunk_sizes.is_empty());
    assert!(state.abort.is_none());
}

#[test]
fn callbacks_without_an_active_sink_are_rejected_including_empty_blocks() {
    let mut state = CallbackState {
        sink: None,
        abort: None,
    };
    assert_eq!(send_chunk(&mut state, b""), -1);
    assert_eq!(send_chunk(&mut state, b"data"), -1);
}

#[test]
fn sink_panics_do_not_unwind_across_the_ffi_callback() {
    for chunk in [b"".as_slice(), b"data".as_slice()] {
        let mut sink = RecordingSink {
            panic_on_write: true,
            ..Default::default()
        };
        let mut state = callback_state(&mut sink);
        assert_eq!(send_chunk(&mut state, chunk), -1);
    }
}
