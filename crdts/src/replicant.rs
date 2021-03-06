use serde::{Deserialize, Serialize};
use sodiumoxide::crypto::sign;
use std::cmp::Ordering;
use std::cmp::Ordering::*;
use std::collections::HashMap;
use std::fmt;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

pub type Time = Duration;
pub type UserPubKey = sign::ed25519::PublicKey;
pub type UserSecKey = sign::ed25519::SecretKey;
pub type Signature = sign::ed25519::Signature;
pub type Pun = u32;
pub type Id = uuid::Uuid;

/// The `Operation` contains all the information needed to apply an operation to a CRDT.
/// This includes a bunch of useful metadata like when it was created, proof of who created it,
/// etc.
///
/// This is split into a couple different structs for ease of storage.  
#[derive(Debug, Hash, Serialize, Deserialize, Clone, Copy, Eq, PartialEq, Ord, PartialOrd)]
pub struct Operation<T> {
    pub user_pub_key: UserPubKey,
    pub data: OperationSigned<T>,
}

#[derive(Debug, Hash, Serialize, Deserialize, Clone, Copy, Eq, PartialEq, Ord, PartialOrd)]
pub struct OperationSigned<T> {
    signature: Signature,
    payload: OperationCounted<T>,
}

#[derive(Debug, Hash, Serialize, Deserialize, Clone, Copy, Eq, PartialEq, Ord, PartialOrd)]
struct OperationCounted<T> {
    counter: Counter,
    time: Time,
    contents: OperationData<T>,
}

#[derive(Debug, Hash, Serialize, Deserialize, Clone, Copy, Eq, PartialEq, Ord, PartialOrd)]
enum OperationData<T> {
    Initial,
    Desc(T),
}

impl<T> OperationData<T> {
    fn is_initial(&self) -> bool {
        match self {
            OperationData::Initial => true,
            OperationData::Desc(_) => false,
        }
    }
}

// Convenience functions for signing and verifying operations
impl<T: Serialize> OperationCounted<T> {
    fn sign(&self, user_secret_key: &UserSecKey) -> Signature {
        let encoded_payload = bincode::serialize(self)
            .expect("Somehow there was a serialization error. This should not ever happen.");
        sign::sign_detached(&encoded_payload, user_secret_key)
    }

    fn verify_sig(&self, signature: &Signature, user_public_key: &UserPubKey) -> bool {
        let encoded_payload = bincode::serialize(self)
            .expect("Somehow there was a serialization error. This should not ever happen.");
        sign::verify_detached(&signature, &encoded_payload, user_public_key)
    }
}

#[derive(Debug, Serialize, Deserialize, Clone, Eq, PartialEq)]
pub struct Account {
    user_pub_key: UserPubKey,
    user_sec_key: UserSecKey,
}

#[derive(Debug, Serialize, Deserialize, Copy, Clone, Eq, PartialEq, Hash)]
pub struct CRDTInfo<T> {
    id: Id,
    initial_value: T,
}

#[derive(Debug, Serialize, Deserialize, Copy, Clone, Eq, PartialEq, Hash, Ord)]
pub enum Counter {
    Initial(Id),
    Operation(Pun, Signature),
}

impl PartialOrd for Counter {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        match (self, other) {
            (Counter::Initial(_), Counter::Operation(_, _)) => Some(Less),
            (Counter::Operation(_, _), Counter::Initial(_)) => Some(Greater),
            (Counter::Initial(id1), Counter::Initial(id2)) => {
                if id1 == id2 {
                    Some(Equal)
                } else {
                    None
                }
            }
            (Counter::Operation(pun1, sig1), Counter::Operation(pun2, sig2)) => {
                if pun1 == pun2 {
                    if sig1 == sig2 {
                        Some(Equal)
                    } else {
                        None
                    }
                } else {
                    Some(pun1.cmp(pun2))
                }
            }
        }
    }
}

// This is used when determining the filenames
impl fmt::Display for Counter {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            Counter::Initial(_) => write!(f, "_initial"),
            Counter::Operation(count, _) => write!(f, "{:0>6}", count),
        }
    }
}

impl Counter {
    fn increment(&mut self, sig: Signature) {
        match self {
            Counter::Initial(_) => {
                *self = Counter::Operation(0, sig);
            }
            Counter::Operation(count, _) => {
                *self = Counter::Operation(*count + 1, sig);
            }
        }
    }

    fn is_initial(&self) -> bool {
        match self {
            Counter::Initial(_) => true,
            Counter::Operation(_, _) => false,
        }
    }
}

#[derive(Debug, Serialize, Deserialize, Clone, Eq, PartialEq)]
pub struct CRDT<T: Applyable> {
    info: CRDTInfo<T>,
    // StateVector stores the counter value of the last performed operation for every user.
    // With it, we can check whether we've already applied any operation by comparing it's counter
    // value against the one in our state vector.
    // If it's counter is less than ours, it's discarded. If it's exactly ours, it's applied and the
    // counter is incremented. If it's greater than
    // ours, that means we somehow missed an operation. We'll put it in `notYetAppliedOperations` to
    // apply later in case turns up.
    state_vector: HashMap<UserPubKey, Counter>,
    #[serde(bound(
        serialize = "T::Description: Serialize",
        deserialize = "T::Description: Deserialize<'de>"
    ))]
    not_yet_applied_operations:
        HashMap<UserPubKey, HashMap<Counter, OperationSigned<T::Description>>>,
    recently_created_and_applied_operations: HashMap<Counter, Operation<T::Description>>,
    pub value: T,
}

impl<T> CRDT<T>
where
    T: Applyable,
    T: Serialize,
    T::Description: Serialize,
    T::Description: Ord,

    T: std::fmt::Debug,
    T::Description: std::fmt::Debug,
{
    /// Applies an operation description to the CRDT.
    /// This is the same as creating an operation from a description with `create_operation` then applying it with `apply`
    pub fn apply_desc(mut self, account: &Account, desc: T::Description) -> Self {
        let counter = self
            .state_vector
            .entry(account.user_pub_key)
            .or_insert(Counter::Initial(self.info.id))
            .clone();
        let (new_crdt, counter) = if counter.is_initial() {
            let (op, new_counter) = self.create_initial_operation(account);
            let mut new_crdt = self.apply(op.clone());
            new_crdt
                .recently_created_and_applied_operations
                .insert(op.data.payload.counter, op);
            (new_crdt, new_counter)
        } else {
            (self, counter)
        };

        let (op, _) = new_crdt.create_operation_from_description(account, desc, counter);
        let mut new_crdt = new_crdt.apply(op.clone());
        new_crdt
            .recently_created_and_applied_operations
            .insert(op.data.payload.counter, op);
        new_crdt
    }

    /// Applies an operation to the CRDT, verifying the signature and checking to make sure it hasn't already been applied
    pub fn apply(mut self, op: Operation<T::Description>) -> Self {
        let user_pub_key = op.user_pub_key;

        // verify that the message is signed by the person who sent it
        // (to make sure nobody is trying to impersonate them)
        if op
            .data
            .payload
            .verify_sig(&op.data.signature, &user_pub_key)
        {
            // The state vector stores the counter of the next operation we expect from every user.
            // Let's see what counter we expect for this user.
            let state_vector_counter = self
                .state_vector
                .entry(user_pub_key)
                .or_insert(Counter::Initial(self.info.id));

            // Let's get the `not_yet_applied_operations` for this user.
            let not_yet_applied_operations = self
                .not_yet_applied_operations
                .entry(user_pub_key)
                .or_default();
            // Now, we insert the operation we're currently working on.
            // This is safe to do because at this point we've already checked the signature
            not_yet_applied_operations.insert(op.data.payload.counter, op.data);

            // `not_yet_applied_operations` is a hashmap to prevent us from adding two operations
            // with the same counter. But now it would be convenient if it were a vector, so we
            // could iterate over it in order.
            let mut not_yet_applied_operations_ordered = not_yet_applied_operations
                .drain()
                .collect::<Vec<(Counter, OperationSigned<T::Description>)>>();
            not_yet_applied_operations_ordered.sort();

            // Any of the operations we can't do right now, we'll store in the hashmap `operations_cant_do_yet`
            let mut operations_cant_do_yet: HashMap<Counter, OperationSigned<T::Description>> =
                HashMap::new();

            // As we iterate over `not_yet_applied_operations`, we are going to be applying the operations to our CRDT's
            // value. It will "accumulate" the changes from all the operations we do, so let's call the current value the
            // accumulator.
            let mut accumulator = self.value;

            // Finally - We iterate over all the operations we still want to do!
            for (counter, op) in not_yet_applied_operations_ordered {
                match (counter).partial_cmp(state_vector_counter) {
                    // If we get an operation who's counter is lower than the one in our state counter, we want to
                    // ignore it (it is a duplicate)
                    Some(Less) => {}
                    // If the operation's counter is greater, that means we're receiving that user's operations
                    // out of order, and need to store the operation to be applied in the future. We store this in
                    // `operations_cant_do_yet` to be merged back into `not_yet_applied_operations` later.
                    Some(Greater) => {
                        operations_cant_do_yet.insert(counter, op);
                    }
                    // If the operation's counter is the same, we want to apply it (and increment that user's
                    // counter in the state vector)
                    Some(Equal) => {
                        state_vector_counter.increment(op.signature);
                        match op.payload.contents {
                            OperationData::Initial => {}
                            OperationData::Desc(desc) => {
                                accumulator = accumulator.apply_without_idempotency_check(
                                    desc,
                                    user_pub_key,
                                    *state_vector_counter,
                                );
                            }
                        };
                    }
                    // It's possible that the counter isn't the same, greater, or lesser, because the signature is 
                    // different. This is probably because someone is trying to rewrite history. I want to have a more
                    // robust solution here in the future but for now I'm just going to fail.
                    None => panic!(
                        "I expected a signature like:\n{:?}\nBut I got:\n{:?}.\nIt's possible that someone has tried to rewrite history.",
                        counter, state_vector_counter
                    ),
                }
            }
            // Now we set `not_yet_applied_operations` to the `operations_cant_do_yet` list we've been building
            *not_yet_applied_operations = operations_cant_do_yet;
            // ...but if it's empty let's just delete the entry from the hashmap to reduce clutter
            if *not_yet_applied_operations == HashMap::new() {
                self.not_yet_applied_operations.remove(&user_pub_key);
            }
            // Finally, we can return the accumulated CRDT!
            CRDT {
                value: accumulator,
                ..self
            }
        } else {
            panic!(
                "I couldn't verify that: {:#?}\nwas actually signed by {:?}",
                &op, &user_pub_key
            )
        }
    }

    fn create_initial_operation(&self, account: &Account) -> (Operation<T::Description>, Counter) {
        let id = self.info.id;
        self.create_operation(account, OperationData::Initial, Counter::Initial(id))
    }

    /// Takes a description and creates an operation
    fn create_operation_from_description(
        &self,
        account: &Account,
        desc: T::Description,
        counter: Counter,
    ) -> (Operation<T::Description>, Counter) {
        self.create_operation(account, OperationData::Desc(desc), counter)
    }

    /// Takes a description and creates an operation
    fn create_operation(
        &self,
        account: &Account,
        op_data: OperationData<T::Description>,
        mut counter: Counter,
    ) -> (Operation<T::Description>, Counter) {
        assert!(
            op_data.is_initial() == counter.is_initial(),
            "Trying to create an operation with the data {:?} but the counter {:?}.",
            op_data,
            counter
        );

        let payload = OperationCounted {
            counter,
            time: SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .expect("Time went backwards"),
            contents: op_data,
        };

        let op = Operation {
            user_pub_key: account.user_pub_key,
            data: OperationSigned {
                signature: payload.sign(&account.user_sec_key),
                payload,
            },
        };

        counter.increment(op.data.signature);

        (op, counter)
    }

    pub fn flush(&mut self) -> HashMap<Counter, Operation<T::Description>> {
        let mut output = HashMap::new();
        std::mem::swap(
            &mut output,
            &mut self.recently_created_and_applied_operations,
        );
        //self.recently_created_and_applied_operations = HashMap::new();
        output
    }
}

pub fn get_random_id() -> Id {
    uuid::Uuid::new_v4()
}

pub fn create_account(user_pub_key: UserPubKey, user_sec_key: UserSecKey) -> Account {
    Account {
        user_pub_key,
        user_sec_key,
    }
}

pub fn create_crdt_info<T: Applyable>(applyable: T, id: Id) -> CRDTInfo<T> {
    CRDTInfo {
        id,
        initial_value: applyable,
    }
}

pub fn create_crdt<T: Applyable>(info: CRDTInfo<T>) -> CRDT<T> {
    CRDT {
        state_vector: HashMap::new(),
        not_yet_applied_operations: HashMap::new(),
        recently_created_and_applied_operations: HashMap::new(),
        value: info.initial_value.clone(),
        info,
    }
}

pub trait Applyable: Clone {
    /// This is the name of the CRDT, mostly for debugging/testing reasons.
    const NAME: &'static str;

    /// This is the type that represents what operations can be done on your CRDT.
    type Description: Clone;

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
    /// Also, if a user does an action, then another action, they will always be applied in that order
    /// (for all peers). But if I do an action and you do an action, the order of application isn't specified.
    fn apply_without_idempotency_check(
        self,
        desc: Self::Description,
        user_pub_key: UserPubKey,
        counter: Counter,
    ) -> Self;
}

/// Nat is a very simple CRDT. It is just a number that can only go up. If I increment it and you increment it,
/// when we merge the result will have been incremented twice.
#[derive(Debug, Serialize, Deserialize, Clone, Copy, Eq, PartialEq, Ord, PartialOrd, Hash)]
pub struct Nat {
    pub value: u32,
}

// This is used when determining the filenames
impl fmt::Display for Nat {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "{}", self.value)
    }
}

impl Default for Nat {
    fn default() -> Self {
        Nat { value: 0 }
    }
}

impl Applyable for Nat {
    const NAME: &'static str = "Nat";

    type Description = u32;

    fn apply_without_idempotency_check(
        self,
        desc: Self::Description,
        _: UserPubKey,
        _: Counter,
    ) -> Self {
        Nat {
            value: self.value.saturating_add(desc),
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

    use pretty_assertions::assert_eq;
    use proptest::prelude::*;

    use CRDT;

    #[test]
    fn apply_desc_for_nats() {
        let account = {
            let (pk, sk): (sign::ed25519::PublicKey, sign::ed25519::SecretKey) =
                sign::gen_keypair();
            create_account(pk, sk)
        };
        let initial = create_crdt(create_crdt_info(Nat::from(0), get_random_id()));

        let new = initial.apply_desc(&account, 3);

        assert_eq!(new.value.value, 3);
    }

    #[test]
    fn basic_nat_test() {
        let vs1 = vec![1, 2, 3, 4, 5];

        let (pk, sk): (sign::ed25519::PublicKey, sign::ed25519::SecretKey) = sign::gen_keypair();
        let mut account = create_account(pk, sk);
        let initial = create_crdt(create_crdt_info(Nat::from(0), get_random_id()));

        let mut do_all = |i: CRDT<Nat>, vs: Vec<u32>| {
            vs.into_iter()
                .fold(i, |acc, desc| acc.apply_desc(&mut account, desc))
        };

        let try1 = do_all(initial, vs1.clone());

        assert_eq!(try1.value.value, vs1.iter().sum::<u32>());
    }

    proptest! {


        #[test]
        fn order_insensitive(vs1 in any::<Vec<u32>>()) {
            if vs1.len() > 0 {
                let (initial, operations) = {
                    let (pk, sk): (sign::ed25519::PublicKey, sign::ed25519::SecretKey) = sign::gen_keypair();
                    let mut account = create_account(pk, sk);
                    let initial = create_crdt(create_crdt_info(Nat::from(0), get_random_id()));


                    let mut operations = vec![];
                    let (op, counter) = initial.create_initial_operation(&mut account);
                    operations.push(op);
                    let mut counter = counter;
                    for desc in vs1 {
                        let (op, new_counter) = initial.create_operation_from_description(&mut account, desc, counter);
                        operations.push(op);
                        counter = new_counter;
                    }
                    (initial, operations)
                };


                let shuffled = {
                    let mut rng = StdRng::seed_from_u64(0);
                    let mut shuffled = operations.clone();
                    shuffled.shuffle(&mut rng);
                    shuffled
                };



                let do_all = |i: CRDT<Nat>, vs: Vec<Operation<u32>>| vs.into_iter().fold(i, CRDT::apply);

                let try1 = do_all(initial.clone(), operations);
                let try2 = do_all(initial.clone(), shuffled);

                prop_assert_eq!(&try1.not_yet_applied_operations, &HashMap::new());
                prop_assert_eq!(&try1, &try2);
            }
        }


        #[test]
        fn idempotent(vs1 in any::<Vec<u32>>()) {

            if vs1.len() > 0 {
                let (initial, operations) = {
                    let (pk, sk): (sign::ed25519::PublicKey, sign::ed25519::SecretKey) = sign::gen_keypair();
                    let mut account = create_account(pk, sk);
                    let initial = create_crdt(create_crdt_info(Nat::from(0), get_random_id()));

                    let mut operations = vec![];
                    let (op, counter) = initial.create_initial_operation(&mut account);
                    operations.push(op);
                    let mut counter = counter;
                    for desc in vs1 {
                        let (op, new_counter) = initial.create_operation_from_description(&mut account, desc, counter);
                        operations.push(op);
                        counter = new_counter;
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

                prop_assert_eq!(&try1.not_yet_applied_operations, &HashMap::new());
                prop_assert_eq!(&try1, &try2);
            }
        }


        #[test]
        fn idempotent_and_order_insensitive(vs1 in any::<Vec<u32>>()) {

            if vs1.len() > 0 {
                let (initial, operations) = {
                    let (pk, sk): (sign::ed25519::PublicKey, sign::ed25519::SecretKey) = sign::gen_keypair();
                    let mut account = create_account(pk, sk);
                    let initial = create_crdt(create_crdt_info(Nat::from(0), get_random_id()));

                    let mut operations = vec![];
                    let (op, counter) = initial.create_initial_operation(&mut account);
                    operations.push(op);
                    let mut counter = counter;
                    for desc in vs1 {
                        let (op, new_counter) = initial.create_operation_from_description(&mut account, desc, counter);
                        operations.push(op);
                        counter = new_counter;
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

                prop_assert_eq!(&try1.not_yet_applied_operations, &HashMap::new());
                prop_assert_eq!(&try1, &try2);
            }
        }

    }
}
