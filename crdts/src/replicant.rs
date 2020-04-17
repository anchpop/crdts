pub trait Semilattice: Clone {
  const NAME: &'static str;
  fn default() -> Self;
  fn join(left: Self, right: Self) -> Self;
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


impl Semilattice for Nat {
  const NAME: &'static str = "Nat";

  fn default() -> Self {
    return Nat { value: 0 };
  }

  fn join(left: Self, right: Self) -> Self {
    Nat {
        value: std::cmp::max(left.value, right.value)
    }
  }
}

impl Nat {
  fn increment(&self, v: u32) -> Self {
    let newValue = self.value.checked_add(v).unwrap_or(u32::MAX);
    Nat {value: newValue}
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

  use Semilattice;

  proptest! {
    #[test]
    fn commutative_simple(v in any::<u32>()) {
      let initial: Nat = Semilattice::default();
      let incremented = initial.increment(v);
      prop_assert_eq!(Semilattice::join(initial, incremented), Semilattice::join(incremented, initial))
    }

    
    #[test]
    fn commutative_many(vs1 in any::<Vec<u32>>()) {
      let vs2 = {
        let mut rng = StdRng::seed_from_u64 (0);
        let mut vs2 = vs1.clone();
        vs2.shuffle(&mut rng);
        vs2
      };

      let initial: Nat = Semilattice::default();

      let try1 = vs1.into_iter().map(Nat::from).fold(initial, Semilattice::join);
      let try2 = vs2.into_iter().map(Nat::from).fold(initial, Semilattice::join);

      prop_assert_eq!(try1, try2)
    }
    
  }
}
