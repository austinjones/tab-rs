use std::{
    clone::Clone,
    fmt::Debug,
    sync::{
        atomic::{AtomicBool, Ordering},
        Arc,
    },
    time::{Duration, Instant},
};

/// Creates an atomic assignment, given an identifier.
///
/// Returns a (retraction, assignment) pair.
///
/// The retraction can be used to cancel the assignment, and retrieve the identifier.
///
/// The assignment can be forwarded, and atomically accepted.
pub fn assignment<T: Debug + Clone>(value: T) -> (Retraction<T>, Assignment<T>) {
    let state = Arc::new(AssignmentState::new(value));
    let assignment = Assignment::new(state.clone());
    let retraction = Retraction::new(state);

    (retraction, assignment)
}

/// The assignment half of the assignment pair.  Can be used to atomically accept the assignment.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Assignment<T: Debug + Clone> {
    state: Arc<AssignmentState<T>>,
}

impl<T: Debug + Clone> Assignment<T> {
    pub(in crate::state::assignment) fn new(state: Arc<AssignmentState<T>>) -> Self {
        Self { state }
    }

    /// Atomically & exclusively accepts the assignment.
    pub fn accept(&self) -> Option<T> {
        self.state.take()
    }
}

/// The retraction half of the assignment pair.  Can be used to atomically retract the assignment, so a new assignment event can be generated.
#[derive(Debug, Clone)]
pub struct Retraction<T: Debug + Clone> {
    state: Arc<AssignmentState<T>>,
}

impl<T: Debug + Clone> Retraction<T> {
    pub(in crate::state::assignment) fn new(state: Arc<AssignmentState<T>>) -> Self {
        Self { state }
    }

    /// Attempts to retract the offer if it was submitted longer ago than the duration
    /// Returns Some if the assignment was retracted.
    pub fn retract_if_expired(&self, duration: Duration) -> Option<T> {
        if self.state.time_since_creation() < duration {
            return None;
        }

        self.state.take()
    }

    pub fn is_taken(&self) -> bool {
        self.state.taken.load(Ordering::SeqCst)
    }

    #[allow(dead_code)]
    /// Attempts to retract the assignment, returning Some if the retraction was successful
    pub fn retract(&self) -> Option<T> {
        self.state.take()
    }
}

/// The internal state shared by the Retraction and Assignment structs.
#[derive(Debug)]
struct AssignmentState<T: Debug + Clone> {
    value: T,
    created: Instant,
    pub(super) taken: AtomicBool,
}

impl<T> PartialEq for AssignmentState<T>
where
    T: PartialEq + Clone + Debug,
{
    fn eq(&self, other: &Self) -> bool {
        self.value == other.value
            && self.created == other.created
            && self.taken.load(Ordering::SeqCst) == other.taken.load(Ordering::SeqCst)
    }
}

impl<T> Eq for AssignmentState<T>
where
    T: Eq + Clone + Debug,
{
    fn assert_receiver_is_total_eq(&self) {}
}

impl<T: Debug + Clone> AssignmentState<T> {
    pub fn new(value: T) -> Self {
        Self {
            value,
            created: Instant::now(),
            taken: AtomicBool::new(false),
        }
    }

    pub fn time_since_creation(&self) -> Duration {
        Instant::now().duration_since(self.created)
    }

    pub fn take(&self) -> Option<T> {
        let taken = self.taken.swap(true, Ordering::SeqCst);

        if !taken {
            Some(self.value.clone())
        } else {
            None
        }
    }
}
