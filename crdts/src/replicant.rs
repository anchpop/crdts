pub trait Semilattice: Clone {
  const NAME: &'static str;
  fn default() -> Self;
  fn join(left: Self, right: Self) -> Self;
}


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
    Nat {value: self.value + v}
  } 
}


#[cfg(test)]
mod tests {
  use super::Semilattice;
  quickcheck! {
    fn commutative(v: u32) -> bool {
      let initial: super::Nat = Semilattice::default();
      let incremented = initial.increment(v);
      Semilattice::join(initial, incremented) == Semilattice::join(incremented, initial) 
    }
  }
}
