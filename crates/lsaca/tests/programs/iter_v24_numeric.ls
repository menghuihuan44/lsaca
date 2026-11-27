// v2.4 numeric iterator terminals: min, max and product over the eight
// supported item types, including empty iterators.

fn main() {
    let i32s: Vec<i32> = vec![4i32, -2, 9, -2];
    println!("i32 min {:?} max {:?}", i32s.clone().into_iter().min(), i32s.into_iter().max());
    let i64s: Vec<i64> = vec![4i64, -2, 9];
    println!("i64 min {:?} max {:?}", i64s.clone().into_iter().min(), i64s.into_iter().max());
    let u32s: Vec<u32> = vec![4u32, 2, 9];
    println!("u32 min {:?} max {:?}", u32s.clone().into_iter().min(), u32s.into_iter().max());
    let u64s: Vec<u64> = vec![4u64, 2, 9];
    println!("u64 min {:?} max {:?}", u64s.clone().into_iter().min(), u64s.into_iter().max());
    let isizes: Vec<isize> = vec![4isize, -8, 1];
    println!("isize min {:?} max {:?}", isizes.clone().into_iter().min(), isizes.into_iter().max());
    let usizes: Vec<usize> = vec![4usize, 2, 9];
    println!("usize min {:?} max {:?}", usizes.clone().into_iter().min(), usizes.into_iter().max());
    let f32s: Vec<f32> = vec![1.5f32, -2.5, 4.5];
    println!("f32 min {:?} max {:?}", f32s.clone().into_iter().min(), f32s.into_iter().max());
    let f64s: Vec<f64> = vec![1.5f64, -2.5, 4.5];
    println!("f64 min {:?} max {:?}", f64s.clone().into_iter().min(), f64s.into_iter().max());

    let p_i32: i32 = vec![2i32, 3, -1].into_iter().product();
    let p_i64: i64 = vec![2i64, 3, 4].into_iter().product();
    let p_u32: u32 = vec![2u32, 3, 4].into_iter().product();
    let p_u64: u64 = vec![2u64, 3, 4].into_iter().product();
    let p_isize: isize = vec![2isize, 3, 4].into_iter().product();
    let p_usize: usize = vec![2usize, 3, 4].into_iter().product();
    let p_f32: f32 = vec![1.5f32, 3.0].into_iter().product();
    let p_f64: f64 = vec![1.5f64, 2.5].into_iter().product();
    println!("product i32 {}", p_i32);
    println!("product i64 {}", p_i64);
    println!("product u32 {}", p_u32);
    println!("product u64 {}", p_u64);
    println!("product isize {}", p_isize);
    println!("product usize {}", p_usize);
    println!("product f32 {}", p_f32);
    println!("product f64 {}", p_f64);

    let empty_i64: Vec<i64> = Vec::new();
    println!("empty min {:?} max {:?}", empty_i64.clone().into_iter().min(), empty_i64.clone().into_iter().max());
    let empty_i32: Vec<i32> = Vec::new();
    let empty_i32_product: i32 = empty_i32.into_iter().product();
    println!("empty i32 product {}", empty_i32_product);
    let empty_vec: Vec<f64> = Vec::new();
    let empty_f64_product: f64 = empty_vec.into_iter().product();
    println!("empty f64 product {}", empty_f64_product);
}
