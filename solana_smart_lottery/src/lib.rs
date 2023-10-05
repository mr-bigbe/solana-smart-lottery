// Solana Smart Lottery
use solana_program::{
    program_error::ProgramError,
    program_pack::{IsInitialized, Pack, Sealed},
    pubkey::Pubkey,
};
use std::collections::{HashMap, HashSet};
use sha2::{Sha256, Digest};
use std::convert::TryInto;

// Custom Error Types
pub enum LotteryError {
    RateLimited = 0,
    Unauthorized,
    AlreadyInitialized,
    OutOfTickets,
    InvalidInput,
    InvalidAdmin,
    InvalidPayoutStructure,
    TimeLockAlreadySet,
    InvalidDrawTime,
    InvalidRandomSeed,
    NoTicketsSold,
    InvalidWalletAddress,
    InvalidDeposit,
    DuplicateTicketPurchase,
    AclViolation,   
}

// Logging Level
#[derive(Debug)]
pub enum LogLevel {
    INFO,
    WARNING,
    ERROR,
}

// Logging Module
pub mod logging {
    use super::LogLevel;
    use solana_program::msg;
    
    pub fn log(log_level: LogLevel, message: &str) {
        if should_log(log_level) {
            let level_str = match log_level {
                LogLevel::INFO => "INFO",
                LogLevel::WARNING => "WARNING",
                LogLevel::ERROR => "ERROR",
            };
            msg!("[{}]: {}", level_str, message);
        }
    }

    pub fn should_log(log_level: LogLevel) -> bool {
        match log_level {
            LogLevel::ERROR => true,
            LogLevel::WARNING => true,
            LogLevel::INFO => false, // Change this to true if you want INFO logs
        }
    }
    pub fn log_transaction(tx_hash: &str, log_level: LogLevel) {
        log(log_level, &format!("Transaction: {}", tx_hash));
    }
    pub fn log_state_change(state_variable: &str, new_value: &str, changed_by: &str) {
        log(LogLevel::INFO, &format!("State change: {} changed to {} by {}", state_variable, new_value, changed_by));
    }
    pub fn log_event(event: &str) {
        log(LogLevel::INFO, &format!("Event: {}", event));
    }
    pub fn log_error(error: &str) {
        log(LogLevel::ERROR, &format!("ERROR: {}", error));
    }
}

// Payout Struct
pub struct PayoutStructure {
    minor: u128,
    grand: u128,
    // Add more as needed
}

// Lottery Struct
pub struct Lottery {
    ticket_data: HashMap<u64, Pubkey>,  // <- Changed to HashMap<u64, Pubkey>
    total_tickets: u64,
    sold_tickets: u64,
    contract_balance: u128,
    ticket_price: u128,
    draw_time: u64,
    random_seed: u64,
    payout_structure: PayoutStructure,
    admin_address: Pubkey,
    rate_limit_map: HashMap<Pubkey, u64>,
    acl: HashSet<Pubkey>,
}

impl Lottery {
    // Initialize a new Lottery struct
    pub fn new() -> Lottery {
        Lottery {
            ticket_data: HashMap::new(),
            total_tickets: 0,
            sold_tickets: 0,
            contract_balance: 0,
            ticket_price: 0,
            draw_time: 0,
            random_seed: 0,
            payout_structure: PayoutStructure {
                minor: 0,
                grand: 0,
            },
            admin_address: Pubkey::default(),
            rate_limit_map: HashMap::new(),
            acl: HashSet::new(),
        }   
    }
    
    pub fn validate_admin(&self, admin_address: Pubkey) -> Result<(), ProgramError> {
        if admin_address != self.admin_address && self.admin_address != Pubkey::default() {
            return Err(ProgramError::Custom(LotteryError::InvalidAdmin as u32));
        }
        Ok(())
    }
    // Initialize Immutable Parameters
    pub fn initialize_lottery(
        &mut self,
        total_tickets: u64,
        ticket_price: u128,
        payout_structure: PayoutStructure,
        admin_address: Pubkey,
    ) -> Result<(), ProgramError> {
        if self.total_tickets != 0 {
            return Err(ProgramError::Custom(LotteryError::AlreadyInitialized as u32));
        }
        if total_tickets == 0 || ticket_price == 0 || admin_address == Pubkey::default() {
            return Err(ProgramError::Custom(LotteryError::InvalidInput as u32));
        }
        let total_percentage: u128 = payout_structure.minor + payout_structure.grand;
        if total_percentage > 10000 {
            return Err(ProgramError::Custom(LotteryError::InvalidPayoutStructure as u32));
        }
        self.total_tickets = total_tickets;
        self.ticket_price = ticket_price;
        self.payout_structure = payout_structure;
        self.admin_address = admin_address;
        Ok(())
    }
    // Data Validation
    pub fn validate_data(&self, user_deposit: u128) -> Result<(), ProgramError> {
        if user_deposit < self.ticket_price {
            return Err(ProgramError::Custom(LotteryError::InvalidDeposit as u32));
        }
        Ok(())
    }

    // New method to wrap several existing methods
    pub fn new_ticket(&mut self, user_wallet_address: Pubkey, user_deposit: u128) -> Result<(), ProgramError> {
        self.validate_data(user_deposit)?;
        self.check_availability()?;
        self.log_new_ticket(user_wallet_address);
        self.allocate_tickets_with_u128(user_wallet_address, user_deposit)
    }

    // New Modular Function for logging
    pub fn log_new_ticket(&self, user_wallet_address: Pubkey) {
        self.log(LogLevel::INFO, &format!("New ticket bought by {}", user_wallet_address));
    }

    // Check Availability of Tickets
    pub fn check_availability(&self) -> Result<(), ProgramError> {
        if self.sold_tickets >= self.total_tickets {
            return Err(ProgramError::Custom(LotteryError::OutOfTickets as u32));
        }
        Ok(())
    }

    // Accept User Deposit
    pub fn allocate_tickets_with_u128(&mut self, user_wallet_address: Pubkey, user_deposit: u128) -> Result<(), ProgramError> { 
        self.validate_data(user_deposit)?;
        self.contract_balance += user_deposit;
        Ok(())
    }

    // Allocate Tickets to User
    pub fn allocate_tickets_with_f64(&mut self, user_wallet_address: Pubkey, user_deposit: u128) -> Result<(), ProgramError> {
        if user_wallet_address == Pubkey::default() {
            return Err(ProgramError::Custom(LotteryError::InvalidWalletAddress as u32));
        }
        let num_tickets = user_deposit / self.ticket_price;
        let total_tickets_sold = self.sold_tickets as u128 + num_tickets;
        if total_tickets_sold > self.total_tickets as u128 {
            return Err(ProgramError::Custom(LotteryError::OutOfTickets as u32));
        }
        for _ in 0..num_tickets {
            let ticket_id = self.generate_unique_ticket_id();
            self.ticket_data.insert(ticket_id, user_wallet_address);
            self.sold_tickets += 1;
        }
        Ok(())
    }    
    // Generate Unique Ticket ID
    fn generate_unique_ticket_id(&self) -> u64 {
        let mut hasher = Sha256::new();
        hasher.update(format!("{}{}", std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_secs(), self.sold_tickets));
        let result = hasher.finalize();
        let unique_id = u64::from_be_bytes(result[0..8].try_into().unwrap());
        unique_id
    }
    
    // Activate Time-Lock
    pub fn activate_time_lock(&mut self, predefined_duration: u64) -> Result<(), ProgramError> {
        if self.draw_time != 0 {
            return Err(ProgramError::Custom(LotteryError::TimeLockAlreadySet as u32));
        }
        self.draw_time = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_secs() + predefined_duration;
        Ok(())
    }
    
    // Execute Chainlink VRF (Pseudo-code, actual implementation needed)
    pub fn execute_chainlink_vrf(&mut self) -> Result<(), ProgramError> {
        let current_time = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_secs();
        if self.draw_time == 0 || current_time < self.draw_time {
            return Err(ProgramError::Custom(LotteryError::InvalidDrawTime as u32));
        }
        // Placeholder for Chainlink VRF
        self.random_seed = 123456;  // Replace with actual Chainlink VRF call
        Ok(())
    }
    
    // Sort Tickets
    fn sort_tickets(&mut self) {
        let mut ticket_vec: Vec<_> = self.ticket_data.keys().cloned().collect();
        ticket_vec.sort_by_key(|key| format!("{}{}", key, self.random_seed));
        
        // Reconstructing the HashMap
        let mut sorted_map = HashMap::new();
        for key in ticket_vec {
            if let Some(val) = self.ticket_data.get(&key) {  // Note the dereference here
                sorted_map.insert(key, *val);  // And here
            }
        }
        self.ticket_data = sorted_map;
    }
     // Execute RNG
    pub fn execute_rng(&mut self) -> Result<(), ProgramError> {
        if self.random_seed == 0 {
            return Err(ProgramError::Custom(LotteryError::InvalidRandomSeed as u32));
        }
        self.sort_tickets();
        Ok(())
    }
    // Select Winners
    pub fn select_winners(&self) -> Result<(Vec<u64>, u64), ProgramError> {
        if self.ticket_data.is_empty() {
            return Err(ProgramError::Custom(LotteryError::NoTicketsSold as u32));
        }
        // Changed from self.ticket_data.keys().take() to clone and collect
        let winners = self.ticket_data.keys().cloned().take((self.total_tickets / 10) as usize).collect::<Vec<u64>>();
        let grand_winner = *self.ticket_data.keys().next().unwrap();
        Ok((winners, grand_winner))
    }
    
    // Calculate Prizes
    pub fn calculate_prizes(&self) -> (u128, u128) {  // <- Changed to use u128
        let minor_prize = self.contract_balance * self.payout_structure.minor / 10000;  // <- Changed to use u128
        let grand_prize = self.contract_balance * self.payout_structure.grand / 10000;  // <- Changed to use u128
        (minor_prize, grand_prize)
    }
       // Transfer Winnings (Pseudo-code)
    pub fn transfer_winnings(&mut self, winners: Vec<u64>, grand_winner: u64) -> Result<(), ProgramError> {
        let (minor_prize, grand_prize) = self.calculate_prizes();
        
        // Logic to transfer `minor_prize` to all `winners`
        // Logic to transfer `grand_prize` to `grand_winner`
        
        Ok(())
    }
    // Owner's Fee Collection (Pseudo-code)
    pub fn collect_owner_fee(&mut self) -> Result<(), ProgramError> {
        // Changed from floating point multiplication to integer-based
        let owner_fee = self.contract_balance * 2 / 10000;  
        self.contract_balance -= owner_fee;
        
        // Logic to transfer `owner_fee` to `self.admin_address`
        
        Ok(())
    }
    // Logging
    pub fn log(&self, log_level: LogLevel, message: &str) {
        // sol_log is the Solana logging function. Replace it with your logging function if different.
        sol_log(&format!("{:?}: {}", log_level, message));
    }
    // Log State Change
    pub fn log_state_change(&self, state_variable: &str, new_value: &str, changed_by: Pubkey) {
        self.log(LogLevel::INFO, &format!("State change: {} changed to {} by {}", state_variable, new_value, changed_by));
    }
    // Log Error
    pub fn log_error(&self, error: LotteryError) {
        self.log(LogLevel::ERROR, &format!("ERROR: {:?}", error));
    }
    // Error Response
    pub fn error_response(&self, error: LotteryError) -> Result<(), ProgramError> {
        Err(ProgramError::Custom(error as u32))
    }
    // Rate Limiting
    pub fn rate_limit(&mut self, caller: Pubkey) -> Result<(), ProgramError> {
        let current_time = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_secs();
        let last_time = self.rate_limit_map.entry(caller).or_insert(0);
        if current_time - *last_time < dynamic_rate_limit(caller) {
            return Err(ProgramError::Custom(LotteryError::RateLimited as u32));
        }
        *last_time = current_time;
        Ok(())
    }
    // Access Control Lists
    // Add to ACL
    pub fn add_to_acl(&mut self, caller: Pubkey) -> Result<(), ProgramError> {
        self.acl.insert(caller);
        Ok(())
    }
    // Remove from ACL
    pub fn remove_from_acl(&mut self, caller: Pubkey) -> Result<(), ProgramError> {
        self.acl.remove(&caller);
        Ok(())
    }
}      
