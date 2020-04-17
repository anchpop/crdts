pub trait CRDT: Clone {
  /// This is the name of the CRDT, mostly for debugging/testing reasons.
  const NAME: &'static str;

  /// This is the type that represents what operations can be done on your CRDT.
  type Operation;

  /// This is the function that makes it a CRDT! 
  /// It needs to be order-insensitive and idempotent. 
  /// Order-insensitive means that `a.apply(x).apply(z)` will be equal to `a.apply(z).apply(x)`.
  /// Idempotent means that `a.apply(x)` will be equal to `a.apply(x).apply(x)`.
  /// These two properties make it easy to sync the CRDT's state across the network. Even in a P2P way!
  ///
  /// How it works is simple. If you do an operation, you send it to all your peers. 
  /// If anyone receives an operation they haven't seen before, they send it to all their peers.
  /// Eventually, everyone will get your operation and can incorporate it into their state. 
  /// This means that not everyone's states will be consistent all the time. This is okay because 
  /// eventually they will become consistent. 
  fn apply(self, op: Self::Operation) -> Self;
}


/// Nat is a very simple CRDT.
/// It represents a counter that can be increased (but never decreased).
/// In the case of conflicts, the bigger counter is used.
/// If the counter is initially 0, and I increase it by one, and you increase it by one,
/// once we merge the combined counter will be 1. If you want the combined counter
/// to be 2, you will need a more complex CRDT.
#[derive(Debug, Clone, Copy, Eq, PartialEq, Ord, PartialOrd)]
pub struct Nat {
  pub value: u32,
}

impl CRDT for Nat {
  const NAME: &'static str = "Nat";

  type Operation = u32;

  fn apply(self, op: Self::Operation) -> Self {
    Nat {
        value: self.value.checked_add(op).unwrap_or(u32::MAX)
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
  use rand::SeedableRng;  
  use rand::seq::SliceRandom;

  use proptest::prelude::*;

  use CRDT;

  proptest! {

    #[test]
    fn commutative(vs1 in any::<Vec<u32>>()) {
      let vs2 = {
        let mut rng = StdRng::seed_from_u64(0);
        let mut vs2 = vs1.clone();
        vs2.shuffle(&mut rng);
        vs2
      };

      let initial = Nat::from(0);

      let do_all = |vs: Vec<u32>| vs.into_iter().fold(initial, CRDT::apply);

      let try1 = do_all(vs1);
      let try2 = do_all(vs2);

      prop_assert_eq!(try1, try2)
    }
    
  }
}
