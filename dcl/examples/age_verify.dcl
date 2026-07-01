module AgeVerification

type Credential = {
    age: Field,
    id_hash: Field
}

circuit verify_adult(
    private cred: Credential,
    public threshold: Field
) -> bool {
    assert cred.age >= threshold;
    let computed_hash = poseidon(cred.age, cred.id_hash);
    return computed_hash == cred.id_hash;
}
