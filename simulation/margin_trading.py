#!/usr/bin/env python
# -*- coding:utf-8 -*-
__author__ = 'Marco'


class Pool(object):
    def __init__(self, shares, balance) -> None:
        self.shares: int = shares
        self.balance: int = balance
    
    def __str__(self) -> str:
        return "(%s, %s)" % (self.shares, self.balance)
    
    def shares_to_amount(self, shares: int) -> int:
        if self.shares > 0:
            return shares * self.balance / self.shares
        else:
            return shares
    
    def amount_to_shares(self, amount: int) -> int:
        if self.balance > 0:
            return amount * self.shares / self.balance
        else:
            return amount

class AssetConfig(object):
    def __init__(self, fr) -> None:
        self.fr = fr

class Asset(object):
    def __init__(self, supplied: Pool, borrowed: Pool, fr: float) -> None:
        self.supplied = supplied
        self.borrowed = borrowed
        self.margin_debt = Pool(0, 0)
        self.reserved: int = 0
        self.prot_fee: int = 0
        self.pending_debt: int = 0
        self.margin_position: int = 0
        self.config = AssetConfig(fr)

    def __str__(self) -> str:
        return "supplied: %s, borrowed: %s, margin_debt: %s, pending_debt: %s, margin_position: %s, reserved: %s, prot_fee: %s" % (self.supplied, self.borrowed, self.margin_debt, self.pending_debt, self.margin_position, self.reserved, self.prot_fee)
    
    def available_amount(self) -> int:
        return self.supplied.balance + self.reserved + self.prot_fee -self.borrowed.balance -self.margin_debt.balance -self.pending_debt

'''
<asset_id, Asset>
'''
ASSETS = {}


def global_init_assets():
    ASSETS["usdt.e"] = Asset(Pool(10000, 10000), Pool(5000, 5000), 0.95)
    ASSETS["usdc.e"] = Asset(Pool(10000, 10000), Pool(5000, 5000), 0.95)
    ASSETS["usdt"] = Asset(Pool(10000, 10000), Pool(5000, 5000), 0.95)
    ASSETS["usdc"] = Asset(Pool(10000, 10000), Pool(5000, 5000), 0.95)
    ASSETS["near"] = Asset(Pool(10000, 10000), Pool(5000, 5000), 0.75)


class Account(object):
    def __init__(self, account_id: str) -> None:
        self.account_id = account_id
        self.supplied = {}  # <token_id: str, shares: int>
        self.positions = {}  # <pos_id: str, position: Position>

'''
<account_id, Account>
'''
ACCOUNTS = {}


def global_init_accounts():
    alice = Account("alice")
    alice.supplied = {
        "usdt.e": 5000,
        "usdc.e": 5000,
        "usdt": 5000,
        "usdc": 5000,
        "near": 5000,
    }
    ACCOUNTS["alice"] = alice

    ACCOUNTS["bob"] = Account("bob")


'''
represent those MarginTradingPositions stored on chain
<pos_id, MarginTradingPosition>
'''
STORED_MT = {}


class MarginTradingError(Exception):
    pass


class Config(object):
    def __init__(self) -> None:
        self.min_open_hf = 1.05
        self.max_leverage_rate = 2.0
        self.max_margin_value = float(1000)
    

class Oracle(object):
    def __init__(self) -> None:
        self.asset_prices = {
            "usdt.e": 1.0,
            "usdc.e": 1.0,
            "dai": 1.0,
            "usdt": 1.0,
            "usdc": 1.0,
            "near": 3.0,
        }
    
    def update_price(self, token: str, price: float) -> None:
        self.asset_prices[token] = price


class Dex(object):
    def __init__(self) -> None:
        self.prices = {
            "usdt.e": 1.0,
            "usdc.e": 1.0,
            "dai": 1.0,
            "usdt": 1.0,
            "usdc": 1.0,
            "near": 3.1,
        }
        self.last: int = 0
    
    def update_price(self, token: str, price: float) -> None:
        self.prices[token] = price
    
    def trade(self, token_in: str, amount_in: float, token_out: str, min_amount_out: float) -> bool:
        value_in = self.prices[token_in] * amount_in
        amount_out = value_in / self.prices[token_out]
        if amount_out < min_amount_out:
            print("Dex trade: Fail, %.02f %s in, request %.02f %s out, short: %.02f" % (
                amount_in, token_in, min_amount_out, token_out, min_amount_out - amount_out))
            return False
        else:
            self.last = amount_out
            print("Dex trade: Succ, %.02f %s in, request %.02f %s out, long: %.02f" % (
                amount_in, token_in, min_amount_out, token_out, amount_out - min_amount_out))
            return True
    
    def get_latest_token_out_amount(self) -> int:
        return self.last


class RegularPosition(object):
    def __init__(self) -> None:
        self.collateral = {}  # <token_id, shares>
        self.borrowed = {}  # <token_id, shares>


class MarginTradingPosition(object):

    def __init__(self) -> None:
        # belongs to supply pool of the asset
        self.margin_asset = ""
        self.margin_shares = 0.0
        # belongs to borrowed pool of the asset
        self.debt_asset = ""
        self.debt_shares = 0.0
        # belongs to position pool of the asset
        # and no interests for position
        self.position_asset = ""
        self.position_amount = 0.0
        # pre-open, running, adjusting
        self.stat = "pre-open"

    def __str__(self) -> str:
        return "Status: %s, Margin: (%s, %.02f), Pos: (%s, %.02f), Debt: (%s, %.02f)" % (self.stat, self.margin_asset, self.margin_shares,
                                                                             self.position_asset, self.position_amount, self.debt_asset, self.debt_shares)
    
    def __repr__(self) -> str:
        return "Status: %s, Margin: (%s, %.02f), Pos: (%s, %.02f), Debt: (%s, %.02f)" % (self.stat, self.margin_asset, self.margin_shares,
                                                                             self.position_asset, self.position_amount, self.debt_asset, self.debt_shares)

    def reset(self, margin: str, margin_shares: float, debt: str, debt_shares: float, position: str, position_amount: float) -> None:
        self.margin_asset = margin
        self.margin_shares = margin_shares
        self.debt_asset = debt
        self.debt_shares = debt_shares
        self.position_asset = position
        self.position_amount = position_amount
    
    def get_margin_value(self, oracle: Oracle) -> float:
        asset: Asset = ASSETS[self.margin_asset]
        return asset.supplied.shares_to_amount(self.margin_shares) * oracle.asset_prices[self.margin_asset]

    def get_debt_value(self, oracle: Oracle) -> float:
        asset: Asset = ASSETS[self.debt_asset]
        return asset.margin_debt.shares_to_amount(self.debt_shares) * oracle.asset_prices[self.debt_asset]

    def get_position_value(self, oracle: Oracle) -> float:
        return self.position_amount * oracle.asset_prices[self.position_asset]


    def get_hf(self, oracle: Oracle) -> float:
        numerator = self.get_margin_value(oracle) * ASSETS[self.margin_asset].config.fr + \
            self.get_position_value(oracle) * ASSETS[self.position_asset].config.fr
        denominator = self.get_debt_value(oracle) / ASSETS[self.debt_asset].config.fr
        return numerator/denominator

    def get_pnl(self, oracle: Oracle) -> float:
        pnl = self.get_position_value(oracle) - self.get_debt_value(oracle)
        return pnl

    def get_lr(self, oracle: Oracle) -> float:
        lr = self.get_debt_value(oracle) / self.get_margin_value(oracle)
        return lr

    def status(self, oracle: Oracle) -> None:
        print(self)
        print("HF: %.04f, PnL: %.02f, LR: %.02f" % (self.get_hf(oracle), self.get_pnl(oracle), self.get_lr(oracle)))


'''

'''
def open_position(user_id: str, config: Config, oracle: Oracle, dex: Dex,
                  margin: str, margin_amount: float, 
                  debt: str, debt_amount: float, 
                  position: str, position_amount: float) -> str:
    pos_id = "%s-%s-%s" % (margin, debt, position)
    asset_debt: Asset = ASSETS[debt]
    asset_margin: Asset = ASSETS[margin]
    asset_position: Asset = ASSETS[position]
    account: Account = ACCOUNTS[user_id]

    if pos_id in account.positions:
        raise MarginTradingError("Position already exist")
    
    # step 0: check if pending_debt of debt asset has debt_amount room available
    # pending_debt should less than 20% of available of this asset
    if (debt_amount + asset_debt.pending_debt) * 5 >= asset_debt.available_amount():
         raise MarginTradingError("Pending debt will overflow")

    # step 1: create MTP with special status (pre-open)
    mt = MarginTradingPosition()
    mt.reset(margin, asset_margin.supplied.amount_to_shares(margin_amount), 
             debt, asset_debt.margin_debt.amount_to_shares(debt_amount), 
             position, position_amount)
    print("Step1: created MTP with pre-open status:")
    mt.status(oracle)

    # step 2: check before call dex to trade
    # check if mt is valid
    if mt.get_hf(oracle) < config.min_open_hf:
        raise MarginTradingError("Health factor is too low when open")
    if mt.get_lr(oracle) > config.max_leverage_rate:
        raise MarginTradingError("Leverage rate is too high when open")
    if mt.get_margin_value(oracle) > config.max_margin_value:
        raise MarginTradingError("Margin value is too high when open")
    # check if price from oracle and user differs too much
    position_amount_from_oracle = mt.get_debt_value(oracle) / oracle.asset_prices[position]
    if position_amount_from_oracle < position_amount:
        position_diff = position_amount - position_amount_from_oracle
        if position_diff / position_amount > 0.05:
            print("detect possible ddos attack due to unreasonable position_amount")
            raise MarginTradingError("Unreasonable requested position amount")
    print("Step2: pre-open MTP passes valid check.")

    # step 3: pre-lending debt and trace in pending_debt of the asset
    account.supplied[margin] -= mt.margin_shares
    mt.debt_shares = 0
    mt.position_amount = 0
    account.positions[pos_id] = mt
    asset_debt.pending_debt += debt_amount
    print("Step3: pending_debt updated and pre-open MTP stored on-chain.")

    # step 4: call dex to trade and wait for callback
    print("Step3: Call dex to trade:")
    callback_params = {"user_id": user_id, "pos_id": pos_id, "debt_amount": debt_amount}
    trade_ret = dex.trade(mt.debt_asset, debt_amount, mt.position_asset, position_amount)

    # step 5a: 
    print("Step5a: Processing trading callback:")
    cb_user_id = callback_params["user_id"]
    cb_pos_id = callback_params["pos_id"]

    lookup_user: Account = ACCOUNTS[cb_user_id]
    lookup_mt: MarginTradingPosition = lookup_user.positions[cb_pos_id]
    if trade_ret:
        # nothing to do, as real trading result would be reported through on_notify
        pass
    else:
        lookup_user.supplied[lookup_mt.margin_asset] += lookup_mt.margin_shares
        ASSETS[lookup_mt.debt_asset].pending_debt -= debt_amount
        del lookup_user.positions[cb_pos_id]
        ACCOUNTS[cb_user_id] = lookup_user
        print("Position cancelled.")

    # step 5b: if trade_ret is true, the detailed trading info would be reported through on_notify
    if trade_ret:
        print("Step5b: Processing trading notify:")
        on_notify = {"user_id": user_id, "pos_id": pos_id, "token_in_amount": debt_amount, "token_out_amount": dex.get_latest_token_out_amount()}
        notify_user: Account = ACCOUNTS[on_notify["user_id"]]
        notify_mt: MarginTradingPosition = notify_user.positions[on_notify["pos_id"]]
        
        notify_asset_debt: Asset = ASSETS[notify_mt.debt_asset]
        debt_shares = notify_asset_debt.margin_debt.amount_to_shares(on_notify["token_in_amount"])
        notify_asset_debt.pending_debt -= on_notify["token_in_amount"]
        notify_asset_debt.margin_debt.shares += debt_shares
        notify_asset_debt.margin_debt.balance += on_notify["token_in_amount"]
        notify_asset_pos: Asset = ASSETS[notify_mt.position_asset]
        notify_asset_pos.margin_position += on_notify["token_out_amount"]

        notify_mt.debt_shares = debt_shares
        notify_mt.position_amount = on_notify["token_out_amount"]
        notify_mt.stat = "running"
        notify_user.positions[on_notify["pos_id"]] = notify_mt
        ACCOUNTS[on_notify["user_id"]] = notify_user
        
        print("Position opened.")
        notify_mt.status(oracle)

    return pos_id


'''

'''
def adjust_margin(config: Config, oracle: Oracle, pos_id: str, adjust_direction: int, adjust_amount: int):
    mt: MarginTradingPosition = STORED_MT[pos_id]
    adjust_margin_shares = config.supply_amount_to_share(mt.margin_asset, adjust_amount)
    if adjust_direction == 0:
        # decrease, move asset from margin to user's supply, 
        mt.margin_shares -= adjust_margin_shares
        # TODO: adjust user supply

        # check updated MT is valid or not
        if mt.get_hf(oracle) < config.min_open_hf:
            raise MarginTradingError("Health factor is too low when open")
        if mt.get_lr(oracle) > config.max_leverage_rate:
            raise MarginTradingError("Leverage rate is too high when open")
    else:
        # increase, move asset from user's supply to this pos as margin
        mt.margin_shares += adjust_margin_shares
        # TODO: adjust user supply

    STORED_MT[pos_id]


'''

'''
def increase_position(config: Config, oracle: Oracle, dex: Dex, pos_id: str, 
                    increased_debt_amount: int, increased_position_amount: int, 
                    market_route_id: int):
    mt: MarginTradingPosition = STORED_MT[pos_id]

    # step 1: check if price from oracle and user differs too much
    position_amount_from_oracle = increased_debt_amount * oracle.asset_prices[mt.debt_asset] / oracle.asset_prices[mt.position_asset]
    if position_amount_from_oracle < increased_position_amount:
        position_diff = increased_position_amount - position_amount_from_oracle
        if position_diff / increased_position_amount > 0.05:
            print("detect possible ddos attack due to unreasonable position_amount")
            raise MarginTradingError("Unreasonable requested position amount")

    # step 2: adjust MT
    increased_debt_shares = config.borrow_amount_to_share(mt.debt_asset, increased_debt_amount)
    mt.debt_shares += increased_debt_shares
    mt.position_amount += increased_position_amount

    # step 3: check before call dex to trade
    # check if mt is valid
    if mt.get_hf(oracle) < config.min_open_hf:
        raise MarginTradingError("Health factor is too low when open")
    if mt.get_lr(oracle) > config.max_leverage_rate:
        raise MarginTradingError("Leverage rate is too high when open")
    if mt.get_margin_value(oracle) > config.max_margin_value:
        raise MarginTradingError("Margin value is too high when open")
    # TODO: adjust global asset's margin_position balance 
    # TODO: adjust global margin_debt pool
    mt.stat = "adjusting"
    STORED_MT[pos_id] = mt
    print("Step3: adjusting MTP passes valid check and stored on-chain.")
    
    # step 4: call dex to trade and wait for callback
    print("Step4: Call dex to trade:")
    callback_params = {"position_key": pos_id, "increased_debt_shares": increased_debt_shares, "increased_position_amount": increased_position_amount}
    trade_rslt = dex.trade(mt.debt_asset, increased_debt_amount, mt.position_asset, increased_position_amount)

    # step 5: in callback, update MTP status if trading succeed or cancel MTP if fail
    # if we need cancel MTP, must cancel debt(not repay, means we pay without any interest)
    print("Step5: Processing trading callback:")
    position_key = callback_params["position_key"]
    callback_increased_debt_shares = callback_params["increased_debt_shares"]
    callback_increased_position_amount = callback_params["increased_position_amount"]
    lookup_mt: MarginTradingPosition = STORED_MT[position_key]
    if trade_rslt:
        lookup_mt.stat = "running"
        STORED_MT[position_key] = lookup_mt
        print("Position adjusted.")
        lookup_mt.status(oracle)
    else:
        # TODO: adjust global asset's margin_position balance 
        # TODO: adjust global margin_debt pool
        lookup_mt.debt_shares -= callback_increased_debt_shares
        lookup_mt.position_amount -= callback_increased_position_amount
        lookup_mt.stat = "running"
        STORED_MT[position_key] = lookup_mt
        print("Position adjusting failed.")


'''

'''
def decrease_position(config: Config, oracle: Oracle, dex: Dex, pos_id: str, 
                    decreased_debt_amount: int, decreased_position_amount: int, 
                    market_route_id: int):
    mt: MarginTradingPosition = STORED_MT[pos_id]

    # step 1: check if price from oracle and user differs too much
    position_amount_from_oracle = decreased_debt_amount * oracle.asset_prices[mt.debt_asset] / oracle.asset_prices[mt.position_asset]
    if position_amount_from_oracle < decreased_position_amount:
        position_diff = decreased_position_amount - position_amount_from_oracle
        if position_diff / decreased_position_amount > 0.05:
            print("detect possible ddos attack due to unreasonable position_amount")
            raise MarginTradingError("Unreasonable requested position amount")
    
    # step 2: adjust MT
    decreased_debt_shares = config.borrow_amount_to_share(mt.debt_asset, decreased_debt_amount)
    mt.debt_shares -= decreased_debt_shares
    mt.position_amount -= decreased_position_amount

    # step 3: check before call dex to trade
    if mt.position_amount > 0:
        # check if mt is valid
        if mt.get_hf(oracle) < config.min_open_hf:
            raise MarginTradingError("Health factor is too low when open")
        if mt.get_lr(oracle) > config.max_leverage_rate:
            raise MarginTradingError("Leverage rate is too high when open")
        if mt.get_margin_value(oracle) > config.max_margin_value:
            raise MarginTradingError("Margin value is too high when open")
    # TODO: adjust global asset's margin_position balance 
    # TODO: can NOT adjust global margin_debt pool !!!
    mt.stat = "adjusting"
    STORED_MT[pos_id] = mt
    print("Step3: adjusting MTP passes valid check and stored on-chain.")

    # step 4: call dex to trade and wait for callback
    print("Step4: Call dex to trade:")
    callback_params = {"position_key": pos_id, "decreased_debt_shares": decreased_debt_shares, "decreased_position_amount": decreased_position_amount}
    trade_rslt = dex.trade(mt.debt_asset, decreased_debt_amount, mt.position_asset, decreased_position_amount)

    # step 5: in callback, update MTP status if trading succeed or cancel MTP if fail
    # if we need cancel MTP, must cancel debt(not repay, means we pay without any interest)
    print("Step5: Processing trading callback:")
    position_key = callback_params["position_key"]
    callback_decreased_debt_shares = callback_params["decreased_debt_shares"]
    callback_decreased_position_amount = callback_params["decreased_position_amount"]
    lookup_mt: MarginTradingPosition = STORED_MT[position_key]
    if trade_rslt:
        print("Position adjusted.")
        if lookup_mt.position_amount == 0:
            # if position is 0, close this position 
            # TODO: and remaining debt to regular borrowed,
            # equivalent_amount = Config.debt_share_to_amount(mt.debt_asset, lookup_mt.debt_shares)
            # Asset.margin_debt.shares -= lookup_mt.debt_shares
            # Asset.margin_debt.balance -= equivalent_amount
            # borrowed_share = Config.borrowed_amount_to_share(mt.debt_asset, equivalent_amount)
            # Asset.borrowed.shares += borrowed_share
            # Asset.borrowed.balance += equivalent_amount
            # TODO: adjust User Regular Position to add borrowed shares
            # Account["Alice"].borrowed[mt.debt_asset] += borrowed_share
            # TODO: as margin shares global asset's supply, only need to adjust user regular position to add margin as collateral.
            # Account["Alice"].collateral[mt.margin_asset] += mt.margin_shares
            del STORED_MT[position_key]
            print("Position decreased to 0 and closed.")
        else:
            lookup_mt.stat = "running"
            STORED_MT[position_key] = lookup_mt
            lookup_mt.status(oracle)
    else:
        # TODO: adjust global asset's margin_position balance 
        # TODO: adjust global margin_debt pool
        lookup_mt.debt_shares += callback_decreased_debt_shares
        lookup_mt.position_amount += callback_decreased_position_amount
        lookup_mt.stat = "running"
        STORED_MT[position_key] = lookup_mt
        print("Position adjusting failed.")


if __name__ == '__main__':
    print("#########START###########")
    # from margin_trading import * 

    global_init_assets()
    print(ASSETS["usdt.e"])
    global_init_accounts()


    config = Config()
    oracle = Oracle()
    dex = Dex()
    # mt = MarginTradingPosition()
    # mt.reset("usdt",1000,"near",500,"usdt",1500)
    # mt.status(oracle)
    # print()

    print("Try open a short-near position: ------------")
    open_position("alice", config, oracle, dex, "usdt", 1000, "near", 500, "usdt", 1500)
    print()

    exit()

    print("Try increase the position: ------------")
    increase_position(config, oracle, dex, "Alice", 100, 300, 0)
    print()

    print("Try decrease half of the position: ------------")
    decrease_position(config, oracle, dex, "Alice", 300, 900, 0)
    print()

    print("Try close the position: ------------")
    decrease_position(config, oracle, dex, "Alice", 300, 900, 0)
    print()
    
    print("#########-END-###########")



