#[cfg(test)]
mod tests {
    use super::*;
    use soroban_sdk::{
        testutils::{Address as _, AuthorizedFunction, AuthorizedInvocation},
        token::{Client as TokenClient, StellarAssetClient},
        Address, Env, IntoVal,
    };

    // ── Helpers ──────────────────────────────────────────────────────────────

    /// Deploy the KreditLane contract and a mock USDC token, then return both clients
    /// together with pre-funded addresses for admin, client, and creator.
    fn setup() -> (
        Env,
        KreditLaneClient<'static>,
        TokenClient<'static>,
        Address, // admin
        Address, // client
        Address, // creator
    ) {
        let env = Env::default();
        env.mock_all_auths(); // auto-approve all require_auth() calls in tests

        // Deploy a Stellar-native test token (mimics USDC)
        let token_admin = Address::generate(&env);
        let token_contract_id = env.register_stellar_asset_contract(token_admin.clone());
        let token = TokenClient::new(&env, &token_contract_id);
        let token_sac = StellarAssetClient::new(&env, &token_contract_id);

        // Deploy KreditLane
        let contract_id = env.register_contract(None, KreditLane);
        let kl = KreditLaneClient::new(&env, &contract_id);

        // Participants
        let admin = Address::generate(&env);
        let client_addr = Address::generate(&env);
        let creator_addr = Address::generate(&env);

        // Fund the client with 1000 tokens (the escrow budget)
        token_sac.mint(&client_addr, &1_000_0000000i128); // 7 decimal places

        // Initialise contract
        kl.initialize(&admin);

        (env, kl, token, admin, client_addr, creator_addr)
    }

    // ── Test 1: Happy path ───────────────────────────────────────────────────
    /// Full single-milestone lifecycle: create → submit → approve → creator paid.
    #[test]
    fn test_happy_path_single_milestone() {
        let (env, kl, token, _admin, client, creator) = setup();
        let token_id = token.address.clone();

        let escrow_amount = 100_0000000i128; // 100 USDC

        // 1. Client creates a 1-milestone job
        let job_id = kl.create_job(&client, &creator, &token_id, &escrow_amount, &1u32);

        // Contract should now hold the escrowed funds
        assert_eq!(
            token.balance(&kl.address),
            escrow_amount,
            "contract should hold escrowed amount"
        );

        // 2. Creator submits work
        kl.submit_work(&creator, &job_id);

        // 3. Client approves → full amount released to creator
        let creator_balance_before = token.balance(&creator);
        kl.approve_milestone(&client, &job_id);
        let creator_balance_after = token.balance(&creator);

        assert_eq!(
            creator_balance_after - creator_balance_before,
            escrow_amount,
            "creator should receive full escrow on approval"
        );

        // Job should be marked Completed
        let job = kl.get_job(&job_id);
        assert_eq!(job.status, JobStatus::Completed);
    }

    // ── Test 2: Edge case – wrong caller cannot approve ──────────────────────
    /// A random address (not the original client) must not be able to approve a milestone.
    #[test]
    #[should_panic(expected = "caller is not the job client")]
    fn test_wrong_caller_cannot_approve() {
        let (env, kl, token, _admin, client, creator) = setup();
        let token_id = token.address.clone();
        let impostor = Address::generate(&env);

        let job_id = kl.create_job(&client, &creator, &token_id, &50_0000000i128, &1u32);
        kl.submit_work(&creator, &job_id);

        // Impostor attempts to approve — must panic
        kl.approve_milestone(&impostor, &job_id);
    }

    // ── Test 3: State verification after multi-milestone partial release ─────
    /// 3-milestone job: after first approval storage shows 1 milestone released
    /// and status is back to Open (waiting for next submission).
    #[test]
    fn test_state_after_first_of_three_milestones() {
        let (env, kl, token, _admin, client, creator) = setup();
        let token_id = token.address.clone();

        let escrow_amount = 300_0000000i128; // 300 USDC split into 3 milestones

        let job_id = kl.create_job(&client, &creator, &token_id, &escrow_amount, &3u32);
        kl.submit_work(&creator, &job_id);
        kl.approve_milestone(&client, &job_id);

        let job = kl.get_job(&job_id);

        assert_eq!(job.milestones_released, 1, "one milestone should be released");
        assert_eq!(
            job.status,
            JobStatus::Open,
            "job should be Open awaiting next submission"
        );

        // Creator should have received exactly 1/3 of the escrow
        assert_eq!(
            token.balance(&creator),
            100_0000000i128,
            "creator gets 100 USDC after first milestone"
        );
    }

    // ── Test 4: Dispute + admin refunds client ───────────────────────────────
    /// Client raises dispute → admin resolves in client's favour → client reimbursed.
    #[test]
    fn test_dispute_resolved_for_client() {
        let (_env, kl, token, admin, client, creator) = setup();
        let token_id = token.address.clone();

        let escrow_amount = 200_0000000i128;
        let client_balance_before = token.balance(&client);

        let job_id = kl.create_job(&client, &creator, &token_id, &escrow_amount, &1u32);
        kl.submit_work(&creator, &job_id);
        kl.raise_dispute(&client, &job_id);

        let job = kl.get_job(&job_id);
        assert_eq!(job.status, JobStatus::Disputed);

        // Admin refunds client
        kl.resolve_dispute(&admin, &job_id, &true);

        let client_balance_after = token.balance(&client);
        assert_eq!(
            client_balance_after,
            client_balance_before,
            "client should be fully reimbursed after dispute"
        );

        let job = kl.get_job(&job_id);
        assert_eq!(job.status, JobStatus::Refunded);
    }

    // ── Test 5: Cannot submit work on a Completed job ────────────────────────
    /// Once a job is Completed it is immutable — further submit_work calls must panic.
    #[test]
    #[should_panic(expected = "job is not in Open state")]
    fn test_cannot_resubmit_completed_job() {
        let (_env, kl, token, _admin, client, creator) = setup();
        let token_id = token.address.clone();

        let job_id = kl.create_job(&client, &creator, &token_id, &50_0000000i128, &1u32);
        kl.submit_work(&creator, &job_id);
        kl.approve_milestone(&client, &job_id); // job is now Completed

        // Attempting to submit again must panic
        kl.submit_work(&creator, &job_id);
    }
}