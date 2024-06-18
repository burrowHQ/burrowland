// unify all error content for whole project
// sys related
pub const E000_ALREADY_INIT: &str = "E000: already initialized";
pub const E001_PROMISE_RESULT_COUNT_INVALID: &str = "E001: promise result count invalid";
pub const E002_NOT_ALLOWED: &str = "E002: not allowed for the caller";
pub const E003_NOT_INIT: &str = "E003: not initialized";
pub const E004_CONTRACT_PAUSED: &str = "E004: contract paused";
pub const E005_NOT_IMPLEMENTED: &str = "E005: not implemented";
pub const E006_INVALID_OPERATOR: &str = "E006: invalid operator";
pub const E007_INVALID_PROTOCOL_FEE_RATE: &str = "E007: invalid protocol fee rate";
pub const E008_ALREADY_ACCEPTED: &str = "E008: already accepted";
pub const E009_INVALID_FROZEN_TOKEN: &str = "E009: invalid frozen token";
pub const E010_INCLUDE_FROZEN_TOKEN: &str = "E010: include frozen token";
pub const E011_INVALID_VIP_USER_DISCOUNT: &str = "E011: invalid vip user discount";
pub const E012_UNSUPPORTED_TOKEN: &str = "E012: unsupported token";

// account related
pub const E100_ACC_NOT_REGISTERED: &str = "E100: account not registered";
pub const E101_INSUFFICIENT_BALANCE: &str = "E101: insufficient balance";
pub const E102_INSUFFICIENT_STORAGE: &str = "E102: insufficient storage";
pub const E103_STILL_HAS_REWARD: &str = "E103: still has reward";
pub const E104_INSUFFICIENT_DEPOSIT: &str = "E104: insufficient deposit";
pub const E105_ASSET_COUNT_EXCEEDED: &str = "E105: asset count exceeded";
pub const E106_MFT_ASSET_COUNT_EXCEEDED: &str = "E106: mft asset count exceeded";
pub const E107_NOT_ENOUGH_STORAGE_FOR_SLOTS: &str = "E107: not enough storage for slots";

//liquidity, point related
pub const E200_INVALID_ENDPOINT: &str = "E200: invalid endpoint";
pub const E201_INVALID_SQRT_PRICE: &str = "E201: invalid sqrt price";
pub const E202_ILLEGAL_POINT: &str = "E202: illegal point";
pub const E203_LIQUIDITY_OVERFLOW: &str = "E203: liquidity overflow";
pub const E204_SLIPPAGE_ERR: &str = "E204: slippage error"; 
pub const E205_INVALID_DESIRE_AMOUNT: &str = "E205: invalid desire amount";
pub const E207_LIQUIDITY_NOT_FOUND: &str = "E207: liquidity not found";
pub const E208_INTERNAL_ERR1: &str = "E208: loc_pt > right_point";
pub const E209_INTERNAL_ERR2: &str = "E209: loc_pt <= left_point";
pub const E210_INTERNAL_ERR3: &str = "E210: loc_pt < left_point";
pub const E211_INTERNAL_ERR4: &str = "E211: loc_pt >= right_point";
pub const E212_INVALID_OUTPUT_TOKEN: &str = "E212: invalid output token";
pub const E213_INVALID_INPUT_TOKEN: &str = "E213: invalid input token";
pub const E214_INVALID_LIQUIDITY: &str = "E214: invalid liquidity";
pub const E215_NOT_LIQUIDITY_OWNER: &str = "E215: not liquidity owner";
pub const E216_INVALID_LPT_LIST: &str = "E216: invalid lpt list";
pub const E218_USER_LIQUIDITY_IS_MINING: &str = "E218: user liquidity is mining";
pub const E219_USER_LIQUIDITY_IS_NOT_MINING: &str = "E219: user liquidity is not mining";
pub const E220_LIQUIDITY_DUPLICATE: &str = "E220: liquidity duplicate";

// limit order related
pub const E300_NOT_ORDER_OWNER: &str = "E300: not order owner";
pub const E301_ACTIVE_ORDER_ALREADY_EXIST: &str = "E301: active order already exist";
pub const E303_ILLEGAL_BUY_TOKEN: &str = "E303: illegal buy token";
pub const E304_ORDER_NOT_FOUND: &str = "E304: order not found";
pub const E305_INVALID_SELLING_TOKEN_ID: &str = "E305: invalid selling token id";
pub const E306_INVALID_CLIENT_ID: &str = "E306: invalid client id";
pub const E307_INVALID_SELLING_AMOUNT: &str = "E307: invalid selling amount";

// pool related
pub const E400_INVALID_POOL_ID: &str = "E400: invalid pool id";
pub const E401_SAME_TOKENS: &str = "E401: same tokens";
pub const E402_ILLEGAL_FEE: &str = "E402: illegal fee";
pub const E403_POOL_NOT_EXIST: &str = "E403: pool not exist";
pub const E404_INVALID_POOL_IDS: &str = "E404: invalid pool ids";
pub const E405_POOL_ALREADY_EXIST: &str = "E405: pool already exist";
pub const E406_POOL_PAUSED: &str = "E406: pool paused";


// NFT
pub const E500_NOT_NFT_OWNER: &str = "E500: not nft owner";
pub const E501_MORE_GAS_IS_REQUIRED: &str = "E501: more gas is required";
pub const E502_CANNOT_PROVIDE_LIMIT_OF_ZERO: &str = "E502: cannot provide limit of 0";
pub const E503_OUT_OF_BOUND: &str = "E503: Out of bounds, please use a smaller from_index";
pub const E504_EXCEED_MAX_APPROVAL_COUNT: &str = "E504: Exceed max approval count";
pub const E505_SENDER_NOT_APPROVED: &str = "E505: Sender not approved";
pub const E506_FORBIDDEN_SELF_TRANSFER: &str = "E506: Self transfer is forbidden";

// misc
pub const E600_INVALID_MSG: &str = "E600: invalid msg";
pub const E601_NEED_DEPOSIT_AT_LEAST_ONE_YOCTO: &str = "E601: Requires attached deposit of at least 1 yoctoNEAR";

// MFT
pub const E700_INVALID_FARMING_TYPE: &str = "E700: invalid farming type";
pub const E701_LIQUIDITY_TOO_SMALL: &str = "E701: liquidity too small";
pub const E702_MFT_SUPPLY_OVERFLOWING: &str = "E702: mft supply overflowing";
pub const E703_SENDER_NOT_FARMING_ACCOUNTID: &str = "E703: sender not farming account id";
pub const E704_RECEIVER_NOT_FARMING_ACCOUNTID: &str = "E704: receiver not farming account id";
pub const E705_INVALID_V_LIQUIDITY: &str = "E705: invalid v liquidity";
pub const E706_TRANSFER_TO_SELF: &str = "E706: transfer to self";