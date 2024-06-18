use crate::*;

pub fn range_info_to_the_left_of_cp(
    pool: &Pool,
    left_point: i32,
    right_point: i32,
    ret: &mut HashMap<i32, RangeInfo>
) {
    let mut liquidity = pool.liquidity;
    if left_point != pool.current_point {
        if let Some(mut current_point) = pool.slot_bitmap.get_nearest_right_valued_slot(pool.current_point, pool.point_delta, left_point / pool.point_delta){
            while current_point < left_point {
                if pool.point_info.has_active_liquidity(current_point, pool.point_delta) {
                    let liquidity_data = pool.point_info.get_liquidity_data(current_point);
                    if liquidity_data.liquidity_delta > 0 {
                        liquidity += liquidity_data.liquidity_delta as u128;
                    } else {
                        liquidity -= (-liquidity_data.liquidity_delta) as u128;
                    }
                }
                current_point = match pool.slot_bitmap.get_nearest_right_valued_slot(current_point, pool.point_delta, left_point / pool.point_delta) {
                    Some(point) => point, 
                    None => { break; }
                };
            }
        }
    }

    let mut current_point = left_point;
    let mut range_left_point = left_point;
    while current_point < right_point {
        let range_right_point = match pool.slot_bitmap.get_nearest_right_valued_slot(current_point, pool.point_delta, right_point / pool.point_delta) {
            Some(point) => point, 
            None => { right_point }
        };
        if pool.point_info.has_active_liquidity(range_right_point, pool.point_delta) {
            if range_left_point != left_point {
                let liquidity_data = pool.point_info.get_liquidity_data(range_left_point);
                if liquidity_data.liquidity_delta > 0 {
                    liquidity += liquidity_data.liquidity_delta as u128;
                } else {
                    liquidity -= (-liquidity_data.liquidity_delta) as u128;
                }
            }
            ret.insert(range_left_point, RangeInfo { 
                left_point: range_left_point, 
                right_point: if range_right_point < right_point {range_right_point} else {right_point}, 
                amount_l: liquidity.into() 
            });
            range_left_point = range_right_point
        } else if range_right_point == right_point {
            if range_left_point != left_point {
                let liquidity_data = pool.point_info.get_liquidity_data(range_left_point);
                if liquidity_data.liquidity_delta > 0 {
                    liquidity += liquidity_data.liquidity_delta as u128;
                } else {
                    liquidity -= (-liquidity_data.liquidity_delta) as u128;
                }
            }
            ret.insert(range_left_point, RangeInfo { 
                left_point: range_left_point, 
                right_point, 
                amount_l: liquidity.into() 
            });
        }
        current_point = range_right_point;
    }
}

pub fn range_info_to_the_right_of_cp(
    pool: &Pool,
    left_point: i32,
    right_point: i32,
    ret: &mut HashMap<i32, RangeInfo>
){

    let mut liquidity = pool.liquidity;
    if pool.point_info.has_active_liquidity(pool.current_point, pool.point_delta) {
        let liquidity_data = pool.point_info.get_liquidity_data(pool.current_point);
        if liquidity_data.liquidity_delta > 0 {
            liquidity -= liquidity_data.liquidity_delta as u128;
        } else {
            liquidity += (-liquidity_data.liquidity_delta) as u128;
        }
    }
    if right_point != pool.current_point {
        if let Some(mut current_point) = pool.slot_bitmap.get_nearest_left_valued_slot(pool.current_point - 1, pool.point_delta, right_point / pool.point_delta) {
            while current_point > right_point {
                if pool.point_info.has_active_liquidity(current_point, pool.point_delta) {
                    let liquidity_data = pool.point_info.get_liquidity_data(current_point);
                    if liquidity_data.liquidity_delta > 0 {
                        liquidity -= liquidity_data.liquidity_delta as u128;
                    } else {
                        liquidity += (-liquidity_data.liquidity_delta) as u128;
                    }
                }
                current_point = match pool.slot_bitmap.get_nearest_left_valued_slot(current_point - 1, pool.point_delta, right_point / pool.point_delta){
                    Some(point) => point, 
                    None => { break; }
                };
            }
        }
    }

    let mut current_point = right_point;
    let mut range_right_point = right_point;
    while current_point > left_point {
        let range_left_point = match pool.slot_bitmap.get_nearest_left_valued_slot(current_point - 1, pool.point_delta, left_point / pool.point_delta){
            Some(point) => point, 
            None => { left_point }
        };
        if pool.point_info.has_active_liquidity(range_left_point, pool.point_delta) {
            if range_right_point != right_point {
                let liquidity_data = pool.point_info.get_liquidity_data(range_right_point);
                if liquidity_data.liquidity_delta > 0 {
                    liquidity -= liquidity_data.liquidity_delta as u128;
                } else {
                    liquidity += (-liquidity_data.liquidity_delta) as u128;
                }
            }
            ret.insert(range_left_point, RangeInfo { 
                left_point: if range_left_point > left_point {range_left_point} else {left_point}, 
                right_point: range_right_point, 
                amount_l: liquidity.into() 
            });
            range_right_point = range_left_point
        } else if range_left_point == left_point {
            if range_right_point != right_point {
                let liquidity_data = pool.point_info.get_liquidity_data(range_right_point);
                if liquidity_data.liquidity_delta > 0 {
                    liquidity -= liquidity_data.liquidity_delta as u128;
                } else {
                    liquidity += (-liquidity_data.liquidity_delta) as u128;
                }
            }
            ret.insert(left_point, RangeInfo { 
                left_point,
                right_point: range_right_point, 
                amount_l: liquidity.into() 
            });
        }
        current_point = range_left_point;
    }
}