use crate::ConversionType;

pub const rational_param_array_i: [f64; 6] = [0.0, 1.0, 0.0, 0.0, 0.0, 1.0];

pub const param_linear_array_i: [f64; 2] = [0.0, 1.0];

pub(crate) fn transform_params(params: Vec<f64>, conv_type: Option<ConversionType>) -> Vec<f64> {
    let mut params = params;
    match conv_type {
        Some(ConversionType::Rational) => {
            if itertools::equal(&params, &rational_param_array_i) {
                params
            } else {
                params.iter_mut().enumerate().map(|(i, x)| {
                    if *x != 0.0 {
                        *x = if i == 2 { -(*x) } else { 1.0 / (*x) };
                    }
                });
                params
            }
        }
        Some(ConversionType::ParametricLinear) => {
            if itertools::equal(&params, &param_linear_array_i) {
                params
            } else {
                params
            }
        }
        _ => params,
    }
}

pub(crate) mod helper {
    use nom::{
        number::complete::{le_f32, le_f64, le_u16, le_u32, le_u64},
        IResult,
    };
    pub(crate) fn read_le_u16(input: &[u8]) -> IResult<&[u8], u16> {
        Ok(le_u16(input)?)
    }
    pub(crate) fn read_le_u32(input: &[u8]) -> IResult<&[u8], u32> {
        Ok(le_u32(input)?)
    }
    pub(crate) fn read_le_u64(input: &[u8]) -> IResult<&[u8], u64> {
        Ok(le_u64(input)?)
    }
    pub(crate) fn read_le_f32(input: &[u8]) -> IResult<&[u8], f32> {
        Ok(le_f32(input)?)
    }
    pub(crate) fn read_le_f64(input: &[u8]) -> IResult<&[u8], f64> {
        Ok(le_f64(input)?)
    }
}
