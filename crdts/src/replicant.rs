pub trait Semilattice: Clone {
  const NAME: &'static str;
  fn join(left: Self, right: Self) -> Self;
}



#[derive(Debug, Clone, Copy)]
pub struct Nat {
  pub value: u32,
}


impl Semilattice for Nat {
  const NAME: &'static str = "Nat";
  fn join(left: Self, right: Self) -> Self {
      Nat {
          value: std::cmp::max(left.value, right.value)
      }
  }
}