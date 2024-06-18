#!/usr/bin/env python
# -*- coding:utf-8 -*-
__author__ = 'Marco'

import json


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


class Config(object):
    def __init__(self) -> None:
        self.min_open_hf = 1.05
        self.max_leverage_rate = 2.0
        self.max_margin_value = float(1000)

CONFIG = Config()


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
            "near": 3.0,
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


class MarginTradingError(Exception):
    pass


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
        # TODO: keep a record in smart contract or leave it to off-line indexer work?
        # keep a record of accumulated overpaid debt, which has 
        # already been automatically deposited into user supply.
        self.trace_accumulated_profit_shares = 0.0

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
        if self.get_debt_value(oracle) > 0:
            numerator = self.get_margin_value(oracle) * ASSETS[self.margin_asset].config.fr + \
                self.get_position_value(oracle) * ASSETS[self.position_asset].config.fr
            denominator = self.get_debt_value(oracle) / ASSETS[self.debt_asset].config.fr
            return numerator/denominator
        else:
            return 10000.0

    def get_pnl(self, oracle: Oracle) -> float:
        pnl = self.get_position_value(oracle) - self.get_debt_value(oracle)
        return pnl

    def get_lr(self, oracle: Oracle) -> float:
        if self.get_debt_value(oracle) > 0:
            if self.get_margin_value(oracle) > 0:
                lr = self.get_debt_value(oracle) / self.get_margin_value(oracle)
                return lr
            else:
                return 10000.0
        else:
            return 0.0

    def status(self, oracle: Oracle) -> None:
        print(self)
        print("HF: %.04f, PnL: %.02f, LR: %.02f" % (self.get_hf(oracle), self.get_pnl(oracle), self.get_lr(oracle)))


'''
simulation of NEP-141 on_resovle_transfer
'''
def on_resolve_transfer(cross_call_rslt: bool, account_id: str, pos_id: str, amount: int, op: str) -> None:
    print("-- Cross-Contract-Call return %s." % cross_call_rslt)
    user: Account = ACCOUNTS[account_id]
    mt: MarginTradingPosition = user.positions[pos_id]
    asset_debt: Asset = ASSETS[mt.debt_asset]
    asset_position: Asset = ASSETS[mt.position_asset]
    if cross_call_rslt:
        # nothing to do, as real trading result would be reported through on_notify
        pass
    else:
        if op == "open":
            user.supplied[mt.margin_asset] += mt.margin_shares
            asset_debt.pending_debt -= amount
            del user.positions[pos_id]
            ACCOUNTS[account_id] = user
            print("-- Position cancelled.")
        elif op == "increase":
            asset_debt.pending_debt -= amount
            mt.stat = "running"
            user.positions[pos_id] = mt
            ACCOUNTS[account_id] = user
            print("-- Increasing Position failed.")
        elif op == "decrease":
            asset_position.margin_position += amount
            mt.stat = "running"
            user.positions[pos_id] = mt
            ACCOUNTS[account_id] = user
            print("-- Decreasing Position failed.")
        else:
            pass



'''
simulation of NEP-141 ft_on_transfer
'''
def ft_on_transfer(token_id: str, sender_id: str, amount: int, msg: str) -> None:
    print("-- %s transfer %s %s token, with msg: %s" % (sender_id, amount, token_id, msg))
    msg_obj = json.loads(msg)

    account: Account = ACCOUNTS[msg_obj["user_id"]]
    mt: MarginTradingPosition = account.positions[msg_obj["pos_id"]]

    if msg_obj["op"] in ["open", "increase"]:
        asset_debt: Asset = ASSETS[mt.debt_asset]
        debt_shares = asset_debt.margin_debt.amount_to_shares(msg_obj["token_in_amount"])
        asset_debt.pending_debt -= msg_obj["token_in_amount"]
        asset_debt.margin_debt.shares += debt_shares
        asset_debt.margin_debt.balance += msg_obj["token_in_amount"]
        asset_pos: Asset = ASSETS[mt.position_asset]
        asset_pos.margin_position += amount

        mt.debt_shares += debt_shares
        mt.position_amount += amount
        mt.stat = "running"
        account.positions[msg_obj["pos_id"]] = mt
        ACCOUNTS[msg_obj["user_id"]] = account
        print("-- %s Position Succeeded." % msg_obj["op"])
    elif msg_obj["op"] == "decrease":
        asset_debt: Asset = ASSETS[mt.debt_asset]
        debt_shares = asset_debt.margin_debt.amount_to_shares(amount)
        repay_shares = min(debt_shares, mt.debt_shares)
        repay_amount = asset_debt.margin_debt.shares_to_amount(repay_shares)
        asset_debt.margin_debt.shares -= repay_shares
        asset_debt.margin_debt.balance -= repay_amount
        
        mt.debt_shares -= repay_shares
        mt.position_amount -= msg_obj["token_in_amount"]
        mt.stat = "running"
        account.positions[msg_obj["pos_id"]] = mt

        # TODO: Shall we also make remaining position goes to user's supply when debt is 0?
        overpay_amount = amount - repay_amount
        if overpay_amount > 0:
            # overpay part goes to user's supply
            overpay_shares = asset_debt.supplied.amount_to_shares(overpay_amount)
            asset_debt.supplied.shares += overpay_shares
            asset_debt.supplied.balance += overpay_amount
            account.supplied[mt.debt_asset] += overpay_shares
            print("-- %s of %s token goes to %s supply as %s shares" % (overpay_amount, mt.debt_asset, msg_obj["user_id"], overpay_shares))

        ACCOUNTS[msg_obj["user_id"]] = account
        print("-- %s Position Succeeded." % msg_obj["op"])
    else:
        pass


'''

'''
def open_position(user_id: str, oracle: Oracle, dex: Dex,
                  margin: str, margin_amount: float, 
                  debt: str, debt_amount: float, 
                  position: str, min_position_amount: float) -> str:
    pos_id = "%s-%s-%s" % (margin, debt, position)
    asset_debt: Asset = ASSETS[debt]
    asset_margin: Asset = ASSETS[margin]
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
             position, min_position_amount)
    print("Step1: created MTP with pre-open status:")
    mt.status(oracle)

    # step 2: check before call dex to trade
    # check if mt is valid
    if mt.get_hf(oracle) < CONFIG.min_open_hf:
        raise MarginTradingError("Health factor is too low when open")
    if mt.get_lr(oracle) > CONFIG.max_leverage_rate:
        raise MarginTradingError("Leverage rate is too high when open")
    if mt.get_margin_value(oracle) > CONFIG.max_margin_value:
        raise MarginTradingError("Margin value is too high when open")
    # check if price from oracle and user differs too much
    position_amount_from_oracle = mt.get_debt_value(oracle) / oracle.asset_prices[position]
    if position_amount_from_oracle < min_position_amount:
        position_diff = min_position_amount - position_amount_from_oracle
        if position_diff / min_position_amount > 0.05:
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
    print("Step4: Call dex to trade:")
    trade_ret = dex.trade(mt.debt_asset, debt_amount, mt.position_asset, min_position_amount)

    # step 5a: 
    print("Step5a: Callback:")
    on_resolve_transfer(trade_ret, user_id, pos_id, debt_amount, "open")

    # step 5b: if trade_ret is true, the detailed trading info would be reported through on_notify
    if trade_ret:
        print("Step5b: Dex call ft_transfer_call to send token_out back to burrow:")
        msg_obj = {"user_id": user_id, "pos_id": pos_id, "token_in_amount": debt_amount, "op": "open"}
        ft_on_transfer(mt.debt_asset, "dex", dex.get_latest_token_out_amount(), json.dumps(msg_obj))
        mt.status(oracle)

    return pos_id


'''

'''
def adjust_margin(user_id: str, oracle: Oracle, pos_id: str, adjust_direction: int, adjust_amount: int):
    account: Account = ACCOUNTS[user_id]
    mt: MarginTradingPosition = account.positions[pos_id]
    asset_margin: Asset = ASSETS[mt.margin_asset]
    adjust_margin_shares = asset_margin.supplied.amount_to_shares(adjust_amount)
    if adjust_direction == 0:
        # decrease, move asset from margin to user's supply, 
        mt.margin_shares -= adjust_margin_shares
        account.supplied[mt.margin_asset] += adjust_amount

        # check updated MT is valid or not
        if mt.get_hf(oracle) < CONFIG.min_open_hf:
            raise MarginTradingError("Health factor is too low when open")
        if mt.get_lr(oracle) > CONFIG.max_leverage_rate:
            raise MarginTradingError("Leverage rate is too high when open")
    else:
        # increase, move asset from user's supply to this pos as margin
        mt.margin_shares += adjust_margin_shares
        account.supplied[mt.margin_asset] -= adjust_amount

    account.positions[pos_id] = mt
    ACCOUNTS[user_id] = account


'''

'''
def increase_position(user_id: str, oracle: Oracle, dex: Dex, pos_id: str, 
                    increased_debt_amount: int, min_increased_position_amount: int, 
                    market_route_id: int):
    account: Account = ACCOUNTS[user_id]
    mt: MarginTradingPosition = account.positions[pos_id]
    asset_debt: Asset = ASSETS[mt.debt_asset]

    # step 0: check if pending_debt of debt asset has debt_amount room available
    # pending_debt should less than 20% of available of this asset
    if (increased_debt_amount + asset_debt.pending_debt) * 5 >= asset_debt.available_amount():
         raise MarginTradingError("Pending debt will overflow")

    # step 1: check if price from oracle and user differs too much
    position_amount_from_oracle = increased_debt_amount * oracle.asset_prices[mt.debt_asset] / oracle.asset_prices[mt.position_asset]
    if position_amount_from_oracle < min_increased_position_amount:
        position_diff = min_increased_position_amount - position_amount_from_oracle
        if position_diff / min_increased_position_amount > 0.05:
            print("detect possible ddos attack due to unreasonable position_amount")
            raise MarginTradingError("Unreasonable requested position amount")

    # step 2: try evaluating MT to see if it is valid.
    increased_debt_shares = asset_debt.margin_debt.amount_to_shares(increased_debt_amount)
    mt.debt_shares += increased_debt_shares
    mt.position_amount += min_increased_position_amount
    if mt.get_hf(oracle) < CONFIG.min_open_hf:
        raise MarginTradingError("Health factor is too low when open")
    if mt.get_lr(oracle) > CONFIG.max_leverage_rate:
        raise MarginTradingError("Leverage rate is too high when open")
    if mt.get_margin_value(oracle) > CONFIG.max_margin_value:
        raise MarginTradingError("Margin value is too high when open")
    print("Step2: evaluated MTP passes valid check.")

    # step 3: restore MTP and make it go to adjusting state
    mt.debt_shares -= increased_debt_shares
    mt.position_amount -= min_increased_position_amount
    mt.stat = "adjusting"
    account.positions[pos_id] = mt
    asset_debt.pending_debt += increased_debt_amount
    print("Step3: MTP goes to adjusting state.")
    
    # step 4: call dex to trade and wait for callback
    print("Step4: Call dex to trade:")
    trade_ret = dex.trade(mt.debt_asset, increased_debt_amount, mt.position_asset, min_increased_position_amount)

    # step 5a: 
    print("Step5a: Callback:")
    on_resolve_transfer(trade_ret, user_id, pos_id, increased_debt_amount, "increase")

    # step 5b: if trade_ret is true, the detailed trading info would be reported through ft_on_transfer
    if trade_ret:
        print("Step5b: Dex call ft_transfer_call to send token_out back to burrow:")
        msg_obj = {"user_id": user_id, "pos_id": pos_id, "token_in_amount": increased_debt_amount, "op": "increase"}
        ft_on_transfer(mt.debt_asset, "dex", dex.get_latest_token_out_amount(), json.dumps(msg_obj))
        mt.status(oracle)


'''

'''
def decrease_position(user_id: str, oracle: Oracle, dex: Dex, pos_id: str, 
                    min_decreased_debt_amount: int, decreased_position_amount: int, 
                    market_route_id: int):
    account: Account = ACCOUNTS[user_id]
    mt: MarginTradingPosition = account.positions[pos_id]
    asset_debt: Asset = ASSETS[mt.debt_asset]
    asset_position: Asset = ASSETS[mt.position_asset]

    # # step 1: check if price from oracle and user differs too much
    # position_amount_from_oracle = min_decreased_debt_amount * oracle.asset_prices[mt.debt_asset] / oracle.asset_prices[mt.position_asset]
    # if position_amount_from_oracle < decreased_position_amount:
    #     position_diff = decreased_position_amount - position_amount_from_oracle
    #     if position_diff / decreased_position_amount > 0.05:
    #         print("detect possible ddos attack due to unreasonable position_amount")
    #         raise MarginTradingError("Unreasonable requested position amount")
    
    # step 2: try evaluating MT to see if it is valid.
    decreased_debt_shares = asset_debt.margin_debt.amount_to_shares(min_decreased_debt_amount)
    mt.debt_shares -= decreased_debt_shares
    mt.position_amount -= decreased_position_amount
    # TODO: seems it is unnecessary in a decreasing scenario
    if mt.position_amount > 0:
        # check if mt is valid
        if mt.get_hf(oracle) < CONFIG.min_open_hf:
            raise MarginTradingError("Health factor is too low when open")
        if mt.get_lr(oracle) > CONFIG.max_leverage_rate:
            raise MarginTradingError("Leverage rate is too high when open")
        if mt.get_margin_value(oracle) > CONFIG.max_margin_value:
            raise MarginTradingError("Margin value is too high when open")
    print("Step2: evaluated MTP passes valid check.")
    
    # step 3: restore MTP and make it to adjusting state
    # Note: can NOT adjust global margin_debt pool in advance!!!
    mt.debt_shares += decreased_debt_shares
    mt.position_amount += decreased_position_amount
    mt.stat = "adjusting"
    account.positions[pos_id] = mt
    asset_position.margin_position -= decreased_position_amount
    print("Step3: MTP goes to adjusting state.")

    # step 4: call dex to trade and wait for callback
    print("Step4: Call dex to trade:")
    # callback_params = {"position_key": pos_id, "decreased_debt_shares": decreased_debt_shares, "decreased_position_amount": decreased_position_amount}
    # trade_ret = dex.trade(mt.debt_asset, decreased_debt_amount, mt.position_asset, decreased_position_amount)
    trade_ret = dex.trade(mt.position_asset, decreased_position_amount, mt.debt_asset, min_decreased_debt_amount)

    # step 5a: 
    print("Step5a: Callback:")
    on_resolve_transfer(trade_ret, user_id, pos_id, decreased_position_amount, "decrease")

    # step 5b: if trade_ret is true, the detailed trading info would be reported through ft_on_transfer
    if trade_ret:
        print("Step5b: Dex call ft_transfer_call to send token_out back to burrow:")
        msg_obj = {"user_id": user_id, "pos_id": pos_id, "token_in_amount": decreased_position_amount, "op": "decrease"}
        ft_on_transfer(mt.debt_asset, "dex", dex.get_latest_token_out_amount(), json.dumps(msg_obj))
        mt.status(oracle)



if __name__ == '__main__':
    print("#########START###########")
    # from margin_trading import * 

    global_init_assets()

    global_init_accounts()

    oracle = Oracle()
    dex = Dex()
    oracle.asset_prices["near"] = 3.0
    dex.prices["near"] = 3.0

    print("Try open a short-near position: ------------")
    debt_amount = 500
    min_position_amount = 0.99 * debt_amount * oracle.asset_prices["near"] / oracle.asset_prices["usdt"]
    open_position("alice", oracle, dex, "usdt", 1000, "near", debt_amount, "usdt", min_position_amount)
    print()

    print("Try increase the position: ------------")
    oracle.asset_prices["near"] = 2.9
    dex.prices["near"] = 2.9
    debt_amount = 100
    min_position_amount = 0.99 * debt_amount * oracle.asset_prices["near"] / oracle.asset_prices["usdt"]
    increase_position("alice", oracle, dex, "usdt-near-usdt", debt_amount, min_position_amount, 0)
    print()
    # Status: running, Margin: (usdt, 1000.00), Pos: (usdt, 1790.00), Debt: (near, 600.00)

    # 
    print("Try decrease half of the position: ------------")
    oracle.asset_prices["near"] = 3.0
    dex.prices["near"] = 3.0
    pos_amount = 790
    min_debt_amount = 0.99 * pos_amount * oracle.asset_prices["usdt"] / oracle.asset_prices["near"]
    decrease_position("alice", oracle, dex, "usdt-near-usdt", min_debt_amount, pos_amount, 0)
    print()
    # Status: running, Margin: (usdt, 1000.00), Pos: (usdt, 1000.00), Debt: (near, 336.67)

    print("Try close the position: ------------")
    oracle.asset_prices["near"] = 2.8
    dex.prices["near"] = 2.8
    pos_amount = 1000
    min_debt_amount = 0.99 * pos_amount * oracle.asset_prices["usdt"] / oracle.asset_prices["near"]
    decrease_position("alice", oracle, dex, "usdt-near-usdt", 336.67, pos_amount, 0)
    print()

    for key,asset in ASSETS.items():
        print(key, asset)
    
    print()
    print("#########-END-###########")



