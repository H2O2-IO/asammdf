use crate::ConversionType;

/// params of `ConversionType::Rational` with transform (self) -> (self)
pub const RATIONAL_PARAM_ARRAY_I: [f64; 6] = [0.0, 1.0, 0.0, 0.0, 0.0, 1.0];
/// params of `ConversionType::ParametricLinear` with transform (self) -> (self)
pub const PARAM_LINEAR_ARRAY_I: [f64; 2] = [0.0, 1.0];
/// Transform params of a CCBlock
pub(crate) fn transform_params(params: Vec<f64>, conv_type: Option<ConversionType>) -> Vec<f64> {
    let mut params = params;
    match conv_type {
        Some(ConversionType::Rational) => {
            if itertools::equal(&params, &RATIONAL_PARAM_ARRAY_I) {
                params
            } else {
                params = params
                    .iter()
                    .enumerate()
                    .map(|(i, x)| {
                        if *x != 0.0 {
                            if i == 2 {
                                -(*x)
                            } else {
                                1.0 / (*x)
                            }
                        } else {
                            *x
                        }
                    })
                    .collect();
                params
            }
        }
        Some(ConversionType::ParametricLinear) => {
            if itertools::equal(&params, &PARAM_LINEAR_ARRAY_I) {
                params
            } else {
                params
            }
        }
        _ => params,
    }
}

/// Commonly helper for parsing
pub(crate) mod helper {
    use nom::{
        bytes::complete::take,
        multi::count,
        number::complete::{le_f32, le_f64, le_i16, le_u16, le_u32, le_u64},
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
    pub(crate) fn read_n_le_f64(input: &[u8], number: usize) -> IResult<&[u8], Vec<f64>> {
        count(le_f64, number)(input)
    }
    pub(crate) fn read_le_i16(input: &[u8]) -> IResult<&[u8], i16> {
        Ok(le_i16(input)?)
    }
    pub(crate) fn read_str(input: &[u8], count: u32) -> IResult<&[u8], String> {
        let (input, result) = take(count)(input)?;
        let result = String::from_utf8_lossy(result)
            .trim_matches('\0')
            .to_string();
        Ok((input, result))
    }
}
