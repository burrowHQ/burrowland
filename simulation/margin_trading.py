#!/usr/bin/env python
# -*- coding:utf-8 -*-
__author__ = 'Marco'


'''
represent those MarginTradingPositions stored on chain
<user_id -> MarginTradingPosition>
'''
STORED_MT = {}


class MarginTradingError(Exception):
    pass


class Config(object):
    def __init__(self) -> None:
        self.fluctuation_rates = {
            "usdt.e": 0.95,
            "usdc.e": 0.95,
            "dai": 0.95,
            "usdt": 0.95,
            "usdc": 0.95,
            "near": 0.75,
        }
        self.assets = {
            "usdt.e": {"supply_share_unit": 1.0, "borrow_share_unit": 1.0},
            "usdc.e": {"supply_share_unit": 1.0, "borrow_share_unit": 1.0},
            "dai": {"supply_share_unit": 1.0, "borrow_share_unit": 1.0},
            "usdt": {"supply_share_unit": 1.0, "borrow_share_unit": 1.0},
            "usdc": {"supply_share_unit": 1.0, "borrow_share_unit": 1.0},
            "near": {"supply_share_unit": 1.0, "borrow_share_unit": 1.0},
        }
        self.min_open_hf = 1.05
        self.max_leverage_rate = 2.0
        self.max_margin_value = float(1000)
    
    def borrow_share_to_amount(self, asset: str, shares: float) -> float:
        return shares * self.assets[asset]["borrow_share_unit"]

    def supply_share_to_amount(self, asset: str, shares: float) -> float:
        return shares * self.assets[asset]["supply_share_unit"]
    
    def borrow_amount_to_share(self, asset: str, amount: float) -> float:
        return amount / self.assets[asset]["borrow_share_unit"]

    def supply_amount_to_share(self, asset: str, amount: float) -> float:
        return amount / self.assets[asset]["supply_share_unit"]
    


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
            print("Dex trade: Succ, %.02f %s in, request %.02f %s out, long: %.02f" % (
                amount_in, token_in, min_amount_out, token_out, amount_out - min_amount_out))
            return True


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
        # pre-open, running
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
    
    def get_margin_value(self, config: Config, oracle: Oracle) -> float:
        return config.supply_share_to_amount(self.margin_asset, self.margin_shares) * oracle.asset_prices[self.margin_asset]

    def get_debt_value(self, config: Config, oracle: Oracle) -> float:
        return config.borrow_share_to_amount(self.debt_asset, self.debt_shares) * oracle.asset_prices[self.debt_asset]

    def get_position_value(self, oracle: Oracle) -> float:
        return self.position_amount * oracle.asset_prices[self.position_asset]


    def get_hf(self, config: Config, oracle: Oracle) -> float:
        numerator = self.get_margin_value(config, oracle) * config.fluctuation_rates[self.margin_asset] + \
            self.get_position_value(oracle) * config.fluctuation_rates[self.position_asset]
        denominator = self.get_debt_value(config, oracle) / config.fluctuation_rates[self.debt_asset]
        return numerator/denominator

    def get_pnl(self, config: Config, oracle: Oracle) -> float:
        pnl = self.get_position_value(oracle) - self.get_debt_value(config, oracle)
        return pnl

    def get_lr(self, config: Config, oracle: Oracle) -> float:
        lr = self.get_debt_value(config, oracle) / self.get_margin_value(config, oracle)
        return lr

    def status(self, config: Config, oracle: Oracle) -> None:
        print(self)
        print("HF: %.04f, PnL: %.02f, LR: %.02f" % (self.get_hf(config, oracle), self.get_pnl(config, oracle), self.get_lr(config, oracle)))


'''

'''
def open_position(user_id: str, config: Config, oracle: Oracle, dex: Dex,
                  margin: str, margin_amount: float, 
                  debt: str, debt_amount: float, 
                  position: str, position_amount: float):
    # step 1: create MTP with special status (pre-open)
    mt = MarginTradingPosition()
    mt.reset(margin, config.supply_amount_to_share(margin, margin_amount), 
             debt, config.borrow_amount_to_share(debt, debt_amount), 
             position, position_amount)

    # step 2: check before call dex to trade
    # check if mt is valid
    if mt.get_hf(config, oracle) < config.min_open_hf:
        raise MarginTradingError("Health factor is too low when open")
    if mt.get_lr(config, oracle) > config.max_leverage_rate:
        raise MarginTradingError("Leverage rate is too high when open")
    if mt.get_margin_value(config, oracle) > config.max_margin_value:
        raise MarginTradingError("Margin value is too high when open")
    # check if price from oracle and user differs too much
    position_amount_from_oracle = mt.get_debt_value(config, oracle) / oracle.asset_prices[position]
    if position_amount_from_oracle < position_amount:
        position_diff = position_amount - position_amount_from_oracle
        if position_diff / position_amount > 0.05:
            print("detect possible ddos attack due to unreasonable position_amount")
            raise MarginTradingError("Unreasonable requested position amount")
    # store mt with special status (pre-open), in case we need revert when trading fail in later block
    # TODO: update global borrowed info of debt asset
    STORED_MT[user_id] = mt

    # step 3: call dex to trade and wait for callback
    callback_params = {"position_key": user_id}
    trade_rslt = dex.trade(mt.debt_asset, 
                           config.borrow_share_to_amount(mt.debt_asset, mt.debt_shares), 
                           mt.position_asset, mt.position_amount)

    # step 4: in callback, update MTP status if trading succeed or cancel MTP if fail
    # if we need cancel MTP, must cancel debt(not repay, means we pay without any interest)
    position_key = callback_params["position_key"]
    lookup_mt = STORED_MT[position_key]
    if trade_rslt:
        # TODO: position should be handled directly with amount,
        # which means it goes to asset's margin_position pool, 
        # to ensure valid close-position action at any time.
        lookup_mt.stat = "running"
        STORED_MT[position_key] = lookup_mt
        print("Position opened:")
        lookup_mt.status(config, oracle)
    else:
        # TODO: cancel mt.debt, return margin to user's regular collateral
        del STORED_MT[position_key]
        print("Position cancelled")
    


if __name__ == '__main__':
    print("#########START###########")
    # from margin_trading import * 

    config = Config()
    oracle = Oracle()
    dex = Dex()
    mt = MarginTradingPosition()
    mt.reset("usdt",1000,"near",500,"usdt",1500)
    mt.status(config, oracle)
    print()

    print("Try open a position: ------------")
    open_position("Alice", config, oracle, dex, "usdt", 1000, "near", 500, "usdt", 1500)

    print("#########-END-###########")



