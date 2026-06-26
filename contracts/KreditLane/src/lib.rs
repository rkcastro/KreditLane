#![no_std]

use soroban_sdk::{
    contract, contractimpl, contracttype, symbol_short,
    token, Address, Env, Symbol, Vec,
};

// ─────────────────────────────────────────────
// Storage key namespaces
// ─────────────────────────────────────────────
const ADMIN: Symbol = symbol_short!("ADMIN");
const JOB_COUNT: Symbol = symbol_short!("JOB_CNT");

// ─────────────────────────────────────────────
// Data types
// ─────────────────────────────────────────────

/// Current lifecycle state of a job escrow
#[contracttype]
#[derive(Clone, PartialEq, Debug)]
pub enum JobStatus {
    Open,       // Funds locked, waiting for creator to submit work
    Submitted,  // Creator marked work as done; awaiting client approval
    Completed,  // Client approved → creator paid
    Disputed,   // Client raised a dispute → admin arbitrates
    Refunded,   // Dispute resolved in client's favour → client refunded
}

/// Full job record stored on-chain
#[contracttype]
#[derive(Clone)]
pub struct Job {
    pub job_id: u64,
    pub client: Address,          // Student / SME paying for work
    pub creator: Address,         // Filipino/SEA freelancer / content creator
    pub token: Address,           // USDC or XLM token contract address
    pub amount: i128,             // Total escrow amount (in token's smallest unit)
    pub milestone_count: u32,     // How many milestones the job is split into
    pub milestones_released: u32, // How many have been paid out so far
    pub status: JobStatus,
}

/// Key used to look up a job by its numeric ID
#[contracttype]
pub enum DataKey {
    Job(u64),
}

// ─────────────────────────────────────────────
// Contract
// ─────────────────────────────────────────────

#[contract]
pub struct KreditLane;

#[contractimpl]
impl KreditLane {
    // ── Initialise ──────────────────────────────────────────────────────────
    /// Call once after deploy. Sets the trusted admin address used for disputes.
    pub fn initialize(env: Env, admin: Address) {
        // Prevent re-initialisation
        if env.storage().instance().has(&ADMIN) {
            panic!("already initialised");
        }
        env.storage().instance().set(&ADMIN, &admin);
        env.storage().instance().set(&JOB_COUNT, &0u64);
    }

    // ── Create Job ───────────────────────────────────────────────────────────
    /// Client locks USDC/XLM into the contract escrow for a creator.
    /// `milestone_count` splits the payout into equal instalments (1–10).
    pub fn create_job(
        env: Env,
        client: Address,
        creator: Address,
        token: Address,
        amount: i128,
        milestone_count: u32,
    ) -> u64 {
        // Client must authorise this call (prevents spoofing)
        client.require_auth();

        if amount <= 0 {
            panic!("amount must be positive");
        }
        if milestone_count == 0 || milestone_count > 10 {
            panic!("milestone_count must be 1–10");
        }

        // Transfer funds from client → contract (escrow)
        let token_client = token::Client::new(&env, &token);
        token_client.transfer(&client, &env.current_contract_address(), &amount);

        // Assign a monotonically increasing job ID
        let job_id: u64 = env.storage().instance().get(&JOB_COUNT).unwrap_or(0) + 1;
        env.storage().instance().set(&JOB_COUNT, &job_id);

        // Persist the job record
        let job = Job {
            job_id,
            client,
            creator,
            token,
            amount,
            milestone_count,
            milestones_released: 0,
            status: JobStatus::Open,
        };
        env.storage()
            .persistent()
            .set(&DataKey::Job(job_id), &job);

        job_id
    }

    // ── Submit Work ──────────────────────────────────────────────────────────
    /// Creator signals that work for the current milestone is ready for review.
    pub fn submit_work(env: Env, creator: Address, job_id: u64) {
        creator.require_auth();

        let mut job: Job = env
            .storage()
            .persistent()
            .get(&DataKey::Job(job_id))
            .expect("job not found");

        if job.creator != creator {
            panic!("caller is not the job creator");
        }
        if job.status != JobStatus::Open {
            panic!("job is not in Open state");
        }

        job.status = JobStatus::Submitted;
        env.storage()
            .persistent()
            .set(&DataKey::Job(job_id), &job);
    }

    // ── Approve Milestone ────────────────────────────────────────────────────
    /// Client approves the submitted milestone → releases 1/N of the escrow to creator.
    /// When all milestones are approved the job moves to Completed.
    pub fn approve_milestone(env: Env, client: Address, job_id: u64) {
        client.require_auth();

        let mut job: Job = env
            .storage()
            .persistent()
            .get(&DataKey::Job(job_id))
            .expect("job not found");

        if job.client != client {
            panic!("caller is not the job client");
        }
        if job.status != JobStatus::Submitted {
            panic!("work has not been submitted yet");
        }

        // Pay out one milestone tranche
        let tranche = job.amount / job.milestone_count as i128;
        let token_client = token::Client::new(&env, &job.token);
        token_client.transfer(&env.current_contract_address(), &job.creator, &tranche);

        job.milestones_released += 1;

        // Advance state: reset to Open for next milestone or mark Completed
        if job.milestones_released >= job.milestone_count {
            job.status = JobStatus::Completed;
        } else {
            job.status = JobStatus::Open; // Creator can submit the next milestone
        }

        env.storage()
            .persistent()
            .set(&DataKey::Job(job_id), &job);
    }

    // ── Raise Dispute ────────────────────────────────────────────────────────
    /// Client raises a dispute if creator's work is unsatisfactory.
    /// Pauses payouts until admin arbitrates.
    pub fn raise_dispute(env: Env, client: Address, job_id: u64) {
        client.require_auth();

        let mut job: Job = env
            .storage()
            .persistent()
            .get(&DataKey::Job(job_id))
            .expect("job not found");

        if job.client != client {
            panic!("caller is not the job client");
        }
        if job.status != JobStatus::Submitted {
            panic!("can only dispute after work is submitted");
        }

        job.status = JobStatus::Disputed;
        env.storage()
            .persistent()
            .set(&DataKey::Job(job_id), &job);
    }

    // ── Resolve Dispute (Admin) ──────────────────────────────────────────────
    /// Admin resolves a dispute.
    /// `refund_client` = true  → remaining escrow returned to client.
    /// `refund_client` = false → remaining escrow released to creator.
    pub fn resolve_dispute(
        env: Env,
        admin: Address,
        job_id: u64,
        refund_client: bool,
    ) {
        admin.require_auth();

        // Verify caller is the registered admin
        let stored_admin: Address = env
            .storage()
            .instance()
            .get(&ADMIN)
            .expect("contract not initialised");
        if stored_admin != admin {
            panic!("caller is not admin");
        }

        let mut job: Job = env
            .storage()
            .persistent()
            .get(&DataKey::Job(job_id))
            .expect("job not found");

        if job.status != JobStatus::Disputed {
            panic!("job is not in Disputed state");
        }

        // Calculate remaining locked funds (milestones already paid are excluded)
        let paid = (job.amount / job.milestone_count as i128) * job.milestones_released as i128;
        let remaining = job.amount - paid;

        let token_client = token::Client::new(&env, &job.token);

        if refund_client {
            token_client.transfer(&env.current_contract_address(), &job.client, &remaining);
            job.status = JobStatus::Refunded;
        } else {
            token_client.transfer(&env.current_contract_address(), &job.creator, &remaining);
            job.status = JobStatus::Completed;
        }

        env.storage()
            .persistent()
            .set(&DataKey::Job(job_id), &job);
    }

    // ── View helpers ─────────────────────────────────────────────────────────

    /// Returns the full Job record for a given job_id.
    pub fn get_job(env: Env, job_id: u64) -> Job {
        env.storage()
            .persistent()
            .get(&DataKey::Job(job_id))
            .expect("job not found")
    }

    /// Returns the total number of jobs ever created.
    pub fn job_count(env: Env) -> u64 {
        env.storage().instance().get(&JOB_COUNT).unwrap_or(0)
    }
}