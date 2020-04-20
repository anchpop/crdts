use std::cmp::Ordering::*;
use std::collections::HashMap;

type Time = u32;
type UserPubKey = u32;
type Counter = u32;
type Signature = u32;

#[derive(Debug, Clone, Copy, Eq, PartialEq, Ord, PartialOrd)]
pub struct Operation<T> {
    user_pub_key: UserPubKey,
    data: OperationData<T>,
}

#[derive(Debug, Clone, Copy, Eq, PartialEq, Ord, PartialOrd)]
struct OperationData<T> {
    counter: Counter,
    time: Time,
    signature: Signature,
    value: T,
}

#[derive(Debug, Clone, Eq, PartialEq)]
pub struct Account {
    user_pub_key: UserPubKey,
    next_counter: Counter,
}

#[derive(Debug, Clone, Eq, PartialEq)]
pub struct CRDT<T: Applyable> {
    account: Account,
    // StateVector stores the counter value of the last performed operation for every user.
    // With it, we can check whether we've already applied any operation by comparing it's counter
    // value against the one in our state vector.
    // If it's counter is less than ours, it's discarded. If it's exactly ours, it's applied and the
    // counter is incremented. If it's greater than
    // ours, that means we somehow missed an operation. We'll put it in `notYetAppliedOperations` to
    // apply later in case turns up.
    state_vector: HashMap<UserPubKey, Counter>,
    not_yet_applied_operations: HashMap<UserPubKey, Vec<OperationData<T::Description>>>,
    value: T,
}

impl<T: Applyable> CRDT<T> {
    fn apply_desc(self, desc: T::Description) -> Self {
        let (new_crdt, op) = self.create_operation(desc);
        new_crdt.apply(op)
    }

    fn apply(mut self, op: Operation<T::Description>) -> Self {
        // @todo: sign operations and check signatures
        let user_pub_key = op.user_pub_key.clone();
        let state_vector_counter = self.state_vector.entry(user_pub_key).or_insert(0);
        let operations_to_attempt = self
            .not_yet_applied_operations
            .entry(user_pub_key)
            .or_default();
        operations_to_attempt.insert(0, op.data);
        operations_to_attempt.sort();

        let mut operations_cant_do_yet: Vec<OperationData<T::Description>> = vec![];
        let mut current_value = self.value;
        for op in operations_to_attempt.drain(..) {
            match (op.counter).cmp(state_vector_counter) {
                Less => {} // Do nothing
                Greater => {
                    // Store to be applied later
                    operations_cant_do_yet.push(op);
                }
                Equal => {
                    // Apply
                    *state_vector_counter += 1;
                    current_value = current_value.apply_without_idempotency_check(Operation {
                        user_pub_key,
                        data: op,
                    });
                }
            }
        }
        *operations_to_attempt = operations_cant_do_yet;
        CRDT {
            value: current_value,
            ..self
        }
    }

    fn create_operation(self, desc: T::Description) -> (Self, Operation<T::Description>) {
        let counter = self.account.next_counter;
        let new_crdt = CRDT {
            account: Account {
                next_counter: counter + 1,
                ..self.account
            },
            ..self
        };
        let op = Operation {
            user_pub_key: new_crdt.account.user_pub_key,
            data: OperationData {
                counter,
                time: 0, // @todo: record times
                signature: 0,
                value: desc,
            },
        };
        (new_crdt, op)
    }
}

fn create_crdt<T: Applyable>(applyable: T, user_pub_key: UserPubKey) -> CRDT<T> {
    CRDT {
        account: Account {
            user_pub_key,
            next_counter: 0,
        },
        state_vector: HashMap::new(),
        not_yet_applied_operations: HashMap::new(),
        value: applyable,
    }
}

pub trait Applyable: Clone + Default {
    /// This is the name of the CRDT, mostly for debugging/testing reasons.
    const NAME: &'static str;

    /// This is the type that represents what operations can be done on your CRDT.
    type Description: Ord;

    /// This is the function that makes it a CRDT!
    /// It has but one restriction: it must be order-insensitive.
    /// Order-insensitive means that `a.apply(x).apply(z) == a.apply(z).apply(x)`.
    ///
    /// If you're familiar with CRDTs, you might expect that the operation should also be
    /// Idempotent. Idempotent means that `a.apply(x)` will be equal to `a.apply(x).apply(x)`.
    /// We actually implement idempotency for you by annotating each operation with a unique
    /// identifier. Before applying, we automatically check if we've already applied something
    /// with the same identifer, and ignore it if so. That means you don't have to worry about it.
    ///
    /// These two properties, order-insensitivity and idempotency make it easy to sync the CRDT's
    /// state across the network. Even in a P2P way!
    ///
    /// How it works is simple. If you do an operation, you send it to all your peers.
    /// If anyone receives an operation they haven't seen before, they send it to all their peers.
    /// Eventually, everyone will get your operation and can incorporate it into their state.
    /// This means that not everyone's states will be consistent all the time. This is okay because
    /// eventually they will become consistent.
    ///
    /// It's called `applyWithoutIdempotencyCheck` because this function shouldn't worry about that.
    /// The `apply` function will take care of it for you. You write `apply_without_idempotency_check`,
    /// then actually use `apply` or `apply_desc`, which call it internally.
    ///
    /// You can depend on a user's action never getting applied to this function twice.
    /// If you do an action, then another action, they will always be applied in that order. But if I
    /// do an action and you do an action, the order of application isn't specified.
    fn apply_without_idempotency_check(self, op: Operation<Self::Description>) -> Self;
}

/// Nat is a very simple CRDT. It is just a number that can only go up. If I increment it and you increment it,
/// when we merge the result will have been incremented twice.
#[derive(Debug, Clone, Copy, Eq, PartialEq, Ord, PartialOrd)]
pub struct Nat {
    pub value: u32,
}

impl Default for Nat {
    fn default() -> Self {
        Nat { value: 0 }
    }
}

impl Applyable for Nat {
    const NAME: &'static str = "Nat";

    type Description = u32;

    fn apply_without_idempotency_check(self, op: Operation<Self::Description>) -> Self {
        Nat {
            value: self
                .value
                .checked_add(op.data.value)
                .unwrap_or(std::u32::MAX),
        }
    }
}

impl From<u32> for Nat {
    fn from(item: u32) -> Self {
        Nat { value: item }
    }
}

impl Into<u32> for Nat {
    fn into(self) -> u32 {
        self.value
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use rand::rngs::StdRng;
    use rand::seq::SliceRandom;
    use rand::Rng;
    use rand::SeedableRng;

    use pretty_assertions::{assert_eq, assert_ne};
    use proptest::prelude::*;

    use CRDT;

    proptest! {

        #[test]
        fn order_insensitive(vs1 in any::<Vec<u32>>()) {
            let vs2 = {
                let mut rng = StdRng::seed_from_u64(0);
                let mut vs2 = vs1.clone();
                vs2.shuffle(&mut rng);
                vs2
            };


            let initial = create_crdt(Nat::from(0), 0);

            let do_all = |initial: CRDT<Nat>, vs: Vec<u32>| vs.into_iter().fold(initial, CRDT::apply_desc);

            let try1 = do_all(initial.clone(), vs1);
            let try2 = do_all(initial.clone(), vs2);

            prop_assert_eq!(try1, try2)
        }

        #[test]
        fn idempotent(vs1 in any::<Vec<u32>>()) {
            if vs1.len() > 0 {
                let (initial, operations) = {
                    let mut initial = create_crdt(Nat::from(0), 0);

                    let mut operations = vec![];
                    for desc in vs1 {
                        let (new, op) = initial.create_operation(desc);
                        initial = new;
                        operations.push(op);
                    }
                    (initial, operations)
                };


                let extended = {
                    let mut rng = StdRng::seed_from_u64(0);
                    let shuffled = {
                        let mut shuffled = operations.clone();
                        shuffled.shuffle(&mut rng);
                        shuffled
                    };
                    let amt_to_repeat: usize = rng.gen_range(0, operations.len());
                    let mut extended = operations.clone();
                    extended.extend_from_slice(&shuffled[..amt_to_repeat]);
                    extended
                };

                let do_all = |i: CRDT<Nat>, vs: Vec<Operation<u32>>| vs.into_iter().fold(i, CRDT::apply);

                let try1 = do_all(initial.clone(), operations);
                let try2 = do_all(initial.clone(), extended);

                prop_assert_eq!(try1, try2)
            }
        }


        #[test]
        fn idempotent_and_order_insensitive(vs1 in any::<Vec<u32>>()) {
            if vs1.len() > 0 {
                let (initial, operations) = {
                    let mut initial = create_crdt(Nat::from(0), 0);

                    let mut operations = vec![];
                    for desc in vs1 {
                        let (new, op) = initial.create_operation(desc);
                        initial = new;
                        operations.push(op);
                    }
                    (initial, operations)
                };


                let extended = {
                    let mut rng = StdRng::seed_from_u64(0);
                    let shuffled = {
                        let mut shuffled = operations.clone();
                        shuffled.shuffle(&mut rng);
                        shuffled
                    };
                    let amt_to_repeat: usize = rng.gen_range(0, operations.len());
                    let mut extended = operations.clone();
                    extended.extend_from_slice(&shuffled[..amt_to_repeat]);
                    extended.shuffle(&mut rng);
                    extended
                };



                let do_all = |i: CRDT<Nat>, vs: Vec<Operation<u32>>| vs.into_iter().fold(i, CRDT::apply);

                let try1 = do_all(initial.clone(), operations);
                let try2 = do_all(initial.clone(), extended);

                prop_assert_eq!(try1, try2)
            }
        }
    }
}
