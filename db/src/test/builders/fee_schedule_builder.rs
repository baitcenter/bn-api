use diesel::prelude::*;
use models::*;

pub struct FeeScheduleBuilder<'a> {
    name: String,
    connection: &'a PgConnection,
}

impl<'a> FeeScheduleBuilder<'a> {
    pub fn new(connection: &PgConnection) -> FeeScheduleBuilder {
        FeeScheduleBuilder {
            connection,
            name: "Name".into(),
        }
    }

    pub fn finish(self) -> FeeSchedule {
        FeeSchedule::create(
            self.name,
            vec![
                NewFeeScheduleRange {
                    min_price: 20,
                    fee: 10,
                },
                NewFeeScheduleRange {
                    min_price: 1000,
                    fee: 100,
                },
            ],
        ).commit(self.connection)
        .unwrap()
    }
}
