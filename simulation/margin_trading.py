#!/usr/bin/env python
# -*- coding:utf-8 -*-
__author__ = 'Marco'


'''
represent those MarginTradingPositions stored on chain
<pos_id -> MarginTradingPosition>
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
We take user margin and lend him the exactly amount of debt to trade on dex,
But the big problem here is, we have to set position_amount as minimum token out, 
in dex trading request, as we couldn't got the exactly number from trading callback.
So, there would have some dust token belong to the protocol.
eg: usdt 1000 (margin), usdt 1500 (debt), -> 500 near (position)
                              ->  ft_transfer_call ->
otherwise, we need to take a long cross-contract call procedure.
(-> 1. deposit 1500 usdt to dex, -> self -> 2. call dex swap -> self -> 3. withdraw from dex -> self)      
'''
def open_position(user_id: str, config: Config, oracle: Oracle, dex: Dex,
                  margin: str, margin_amount: float, 
                  debt: str, debt_amount: float, 
                  position: str, position_amount: float) -> str:
    # step 1: create MTP with special status (pre-open)
    mt = MarginTradingPosition()
    mt.reset(margin, config.supply_amount_to_share(margin, margin_amount), 
             debt, config.borrow_amount_to_share(debt, debt_amount), 
             position, position_amount)
    print("Step1: created MTP with pre-open status:")
    mt.status(config, oracle)

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
    # Asset[mt.debt_asset].margin_debt += mt.debt_shares
    # Account[user_id].supply[margin_asset] -= mt.margin_shares
    STORED_MT[user_id] = mt
    print("Step2: pre-open MTP passes valid check and stored on-chain.")

    # step 3: call dex to trade and wait for callback
    print("Step3: Call dex to trade:")
    callback_params = {"position_key": user_id,}
    trade_rslt = dex.trade(mt.debt_asset, 
                           config.borrow_share_to_amount(mt.debt_asset, mt.debt_shares), 
                           mt.position_asset, mt.position_amount)
    

    # step 4: in callback, update MTP status if trading succeed or cancel MTP if fail
    # if we need cancel MTP, must cancel debt(not repay, means we pay without any interest)
    print("Step4: Processing trading callback:")
    position_key = callback_params["position_key"]
    lookup_mt: MarginTradingPosition = STORED_MT[position_key]
    if trade_rslt:
        # TODO: position should be handled directly with amount,
        # which means it goes to asset's margin_position pool, 
        # to ensure valid close-position action at any time.
        lookup_mt.stat = "running"
        STORED_MT[position_key] = lookup_mt
        print("Position opened.")
        lookup_mt.status(config, oracle)
    else:
        # TODO: cancel mt.debt not repay debt, return margin to user's regular collateral
        # Asset[mt.debt_asset].margin_debt.shares -= mt.debt_shares
        # Asset[mt.debt_asset].margin_debt.balance -= mt.debt_shares * current_share_price
        # Account[user_id].supply[margin_asset] += mt.margin_shares
        del STORED_MT[position_key]
        print("Position cancelled.")
    return position_key


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
        if mt.get_hf(config, oracle) < config.min_open_hf:
            raise MarginTradingError("Health factor is too low when open")
        if mt.get_lr(config, oracle) > config.max_leverage_rate:
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
    if mt.get_hf(config, oracle) < config.min_open_hf:
        raise MarginTradingError("Health factor is too low when open")
    if mt.get_lr(config, oracle) > config.max_leverage_rate:
        raise MarginTradingError("Leverage rate is too high when open")
    if mt.get_margin_value(config, oracle) > config.max_margin_value:
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
        lookup_mt.status(config, oracle)
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
        if mt.get_hf(config, oracle) < config.min_open_hf:
            raise MarginTradingError("Health factor is too low when open")
        if mt.get_lr(config, oracle) > config.max_leverage_rate:
            raise MarginTradingError("Leverage rate is too high when open")
        if mt.get_margin_value(config, oracle) > config.max_margin_value:
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
            lookup_mt.status(config, oracle)
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

    config = Config()
    oracle = Oracle()
    dex = Dex()
    mt = MarginTradingPosition()
    mt.reset("usdt",1000,"near",500,"usdt",1500)
    mt.status(config, oracle)
    print()

    print("Try open a short-near position: ------------")
    open_position("Alice", config, oracle, dex, "usdt", 1000, "near", 500, "usdt", 1500)
    print()

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



