/// taken from https://github.com/tokio-rs/tokio/pull/2522/files

/// Asserts that the expression completes within a given number of milliseconds.
///
/// This will invoke the `panic!` macro if the provided future
/// expression fails to complete within the given number of
/// milliseconds. This macro expands to an `await` and must be
/// invoked inside an async context.
///
/// A default timeout of 50ms is used if no duration is passed.
///
/// # Examples
///
/// ```rust
/// use tab_service_test::assert_completes;
/// use tokio::time::delay_for;
///
/// # let fut =
/// async {
///     // Succeeds because default time is longer than delay.
///     assert_completes!(delay_for(Duration::from_millis(25)));
/// }
/// # ;
/// # let mut runtime = tokio::runtime::Runtime::new().unwrap();
/// # runtime.block_on(fut);
///```
///
/// ```rust,should_panic
/// use tab_service_test::assert_completes;
/// use tokio::time::delay_for;
///
/// # let fut =
/// async {
///     // Fails because timeout is shorter than delay.
///     assert_completes!(delay_for(Duration::from_millis(25)), 10);
/// }
/// # ;
/// # let mut runtime = tokio::runtime::Runtime::new().unwrap();
/// # runtime.block_on(fut);
/// ```
#[macro_export]
macro_rules! assert_completes {
    ($e:expr) => {
        $crate::assert_completes!($e, 50)
    };
    ($e:expr, $time:literal) => {{
        use std::time::Duration;
        use tokio::time::timeout;
        match timeout(Duration::from_millis($time), $e).await {
            Ok(ret) => ret,
            Err(_) => panic!(
                "assertion failed: {} timed out after {} ms",
                stringify!($e),
                $time,
            ),
        }
    }};
}

/// Asserts that the expression does not complete within a given number of milliseconds.
///
/// This will invoke the `panic!` macro if the provided future
/// expression completes within the given number of milliseconds.
/// This macro expands to an `await` and must be invoked inside an
/// async context.
///
///A default timeout of 50ms is used if no duration is passed.
///
/// # Examples
///
/// ```rust,should_panic
/// use tab_service_test::assert_times_out;
/// use tokio::time::delay_for;
///
/// # let fut =
/// async {
///     // Fails because default time is longer than delay.
///     assert_times_out!(delay_for(Duration::from_millis(25)));
/// }
/// # ;
/// # let mut runtime = tokio::runtime::Runtime::new().unwrap();
/// # runtime.block_on(fut);
/// ```
///
/// ```rust
/// use tab_service_test::assert_times_out;
/// use tokio::time::delay_for;
///
/// # let fut =
/// async {
///     // Succeeds because timeout is shorter than delay.
///     assert_times_out!(delay_for(Duration::from_millis(25)), 10);
/// }
/// # ;
/// # let mut runtime = tokio::runtime::Runtime::new().unwrap();
/// # runtime.block_on(fut);
/// ```

#[macro_export]
macro_rules! assert_times_out {
    ($e:expr) => {
        $crate::assert_times_out!($e, 50)
    };
    ($e:expr, $time:literal) => {{
        use std::time::Duration;
        use tokio::time::timeout;
        match timeout(Duration::from_millis($time), $e).await {
            Ok(_) => panic!(
                "assertion failed: {} completed within {} ms",
                stringify!($e),
                $time,
            ),
            Err(err) => err,
        }
    }};
}
