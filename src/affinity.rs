pub fn get_cpu_cores() -> usize {
    let num_cores = core_affinity::get_core_ids();
    if num_cores.is_none() {
        return 0;
    }

    let num_cores = num_cores.unwrap();
    num_cores.len()
}
