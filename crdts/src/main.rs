#[cfg(test)]
#[macro_use]
extern crate quickcheck;


mod replicant;
use replicant::Semilattice;




fn main() {
    let nat = replicant::Nat { value: 3 };




    println!("{}", replicant::Nat::NAME);
}
