module FixedTest

use std::fixed;

circuit compute_interest(
    private principal: Field,
    private rate: Field
) -> Field {
    let interest = fixed::mul(principal, rate);
    return interest;
}
