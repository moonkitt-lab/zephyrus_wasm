pub mod auto_maintenance;
pub mod hydro_queries;
pub mod hydromancer_tribute_data_loader;
pub mod ibc;
pub mod rewards;
pub mod tribute_queries;
pub mod tws;
pub mod validation;
pub mod vectors;
pub mod vessel_assignment;

#[cfg(test)]
mod vectors_test;

#[cfg(test)]
mod auto_maintenance_test;

#[cfg(test)]
mod hydro_queries_test;

#[cfg(test)]
mod tws_test;

#[cfg(test)]
mod validation_test;

#[cfg(test)]
mod vessel_assignment_test;

#[cfg(test)]
mod rewards_test;
