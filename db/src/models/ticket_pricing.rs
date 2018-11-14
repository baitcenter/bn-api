use chrono::NaiveDateTime;
use diesel;
use diesel::dsl::{self, select};
use diesel::prelude::*;
use diesel::sql_types::{Bool, Timestamp, Uuid as dUuid};
use models::{TicketPricingStatus, TicketType};
use schema::{order_items, ticket_pricing};
use std::borrow::Cow;
use utils::errors::*;
use uuid::Uuid;
use validator::*;
use validators::{self, *};

sql_function!(fn ticket_pricing_no_overlapping_periods(id: dUuid, ticket_type_id: dUuid, start_date: Timestamp, end_date: Timestamp, is_box_office_only: Bool) -> Bool);

#[derive(Identifiable, Associations, Queryable, PartialEq, Debug)]
#[belongs_to(TicketType)]
#[table_name = "ticket_pricing"]
pub struct TicketPricing {
    pub id: Uuid,
    ticket_type_id: Uuid,
    pub name: String,
    status: String,
    pub price_in_cents: i64,
    pub start_date: NaiveDateTime,
    pub end_date: NaiveDateTime,
    pub is_box_office_only: bool,
    created_at: NaiveDateTime,
    updated_at: NaiveDateTime,
}

#[derive(AsChangeset, Clone, Default, Deserialize)]
#[table_name = "ticket_pricing"]
pub struct TicketPricingEditableAttributes {
    pub name: Option<String>,
    pub price_in_cents: Option<i64>,
    pub start_date: Option<NaiveDateTime>,
    pub end_date: Option<NaiveDateTime>,
    pub is_box_office_only: Option<bool>,
}

impl TicketPricing {
    pub fn create(
        ticket_type_id: Uuid,
        name: String,
        start_date: NaiveDateTime,
        end_date: NaiveDateTime,
        price_in_cents: i64,
        is_box_office_only: bool,
    ) -> NewTicketPricing {
        NewTicketPricing {
            ticket_type_id,
            name,
            status: TicketPricingStatus::Published.to_string(),
            start_date,
            end_date,
            price_in_cents,
            is_box_office_only,
        }
    }

    pub fn validate_record(
        &self,
        attributes: &TicketPricingEditableAttributes,
    ) -> Result<(), DatabaseError> {
        let validation_errors = validators::append_validation_error(
            Ok(()),
            "ticket_pricing.start_date",
            validators::start_date_valid(
                attributes.start_date.unwrap_or(self.start_date),
                attributes.end_date.unwrap_or(self.end_date),
            ),
        );
        Ok(validation_errors?)
    }

    pub fn update(
        &self,
        attributes: TicketPricingEditableAttributes,
        conn: &PgConnection,
    ) -> Result<TicketPricing, DatabaseError> {
        self.validate_record(&attributes)?;
        diesel::update(self)
            .set((attributes, ticket_pricing::updated_at.eq(dsl::now)))
            .get_result(conn)
            .to_db_error(ErrorCode::UpdateError, "Could not update ticket_pricing")
    }

    pub fn ticket_pricing_no_overlapping_periods(
        id: Uuid,
        ticket_type_id: Uuid,
        start_date: NaiveDateTime,
        end_date: NaiveDateTime,
        is_box_office_only: bool,
        conn: &PgConnection,
    ) -> Result<Result<(), ValidationError>, DatabaseError> {
        let result = select(ticket_pricing_no_overlapping_periods(
            id,
            ticket_type_id,
            start_date,
            end_date,
            is_box_office_only,
        )).get_result::<bool>(conn)
        .to_db_error(
            ErrorCode::UpdateError,
            "Could not confirm periods do not overlap",
        )?;
        if !result {
            let mut validation_error = create_validation_error(
                "ticket_pricing_overlapping_periods",
                "Ticket pricing dates overlap another ticket pricing period",
            );
            validation_error.add_param(Cow::from("ticket_pricing_id"), &id);
            validation_error.add_param(Cow::from("ticket_type_id"), &ticket_type_id);
            validation_error.add_param(Cow::from("start_date"), &start_date);
            validation_error.add_param(Cow::from("end_date"), &end_date);

            return Ok(Err(validation_error));
        }
        Ok(Ok(()))
    }

    pub fn destroy(&self, conn: &PgConnection) -> Result<usize, DatabaseError> {
        //Check if there is any order items linked to this ticket pricing
        let affected_order_count: i64 = order_items::table
            .filter(order_items::ticket_pricing_id.eq(self.id))
            .select(dsl::count(order_items::id))
            .first(conn)
            .to_db_error(ErrorCode::QueryError, "Could not load order_items")?;
        if affected_order_count == 0 as i64 {
            //Ticket pricing is unused -> delete
            diesel::delete(self)
                .execute(conn)
                .to_db_error(ErrorCode::DeleteError, "Error removing ticket pricing")
        } else {
            //Ticket pricing is used -> mark status for deletion
            #[derive(AsChangeset)]
            #[table_name = "ticket_pricing"]
            struct R {
                pub status: String,
            }
            let status_attribute = R {
                status: TicketPricingStatus::Deleted.to_string(),
            };
            diesel::update(self)
                .set((status_attribute, ticket_pricing::updated_at.eq(dsl::now)))
                //.get_result(conn)
                .execute(conn)
                .to_db_error(
                    ErrorCode::UpdateError,
                    "Could not update ticket_pricing status",
                )?;
            Ok(0 as usize)
        }
    }

    pub fn find(id: Uuid, conn: &PgConnection) -> Result<TicketPricing, DatabaseError> {
        ticket_pricing::table
            .find(id)
            .first::<TicketPricing>(conn)
            .to_db_error(ErrorCode::QueryError, "Error loading ticket pricing")
    }

    pub fn status(&self) -> TicketPricingStatus {
        self.status.parse::<TicketPricingStatus>().unwrap()
    }

    pub fn get_current_ticket_pricing(
        ticket_type_id: Uuid,
        conn: &PgConnection,
    ) -> Result<TicketPricing, DatabaseError> {
        let mut price_points = ticket_pricing::table
            //Must use the box office endpoints to add is_box_office_only prices
            .filter(ticket_pricing::is_box_office_only.eq(false))
            .filter(ticket_pricing::ticket_type_id.eq(ticket_type_id))
            .filter(ticket_pricing::start_date.le(dsl::now))
            .filter(ticket_pricing::end_date.gt(dsl::now))
            .load(conn)
            .to_db_error(ErrorCode::QueryError, "Could not load Ticket Pricing")?;

        if price_points.len() > 1 {
            return Err(DatabaseError::new(
                ErrorCode::MultipleResultsWhenOneExpected,
                Some("Expected a single ticket pricing period but multiple were found".to_string()),
            ));
        } else if price_points.len() == 0 {
            return Err(DatabaseError::new(
                ErrorCode::NoResults,
                Some("No ticket pricing found".to_string()),
            ));
        }

        price_points.pop().ok_or(DatabaseError::new(
            ErrorCode::NoResults,
            Some("No ticket pricing found".to_string()),
        ))
    }
}

#[derive(Clone, Insertable)]
#[table_name = "ticket_pricing"]
pub struct NewTicketPricing {
    ticket_type_id: Uuid,
    name: String,
    status: String,
    price_in_cents: i64,
    is_box_office_only: bool,
    pub start_date: NaiveDateTime,
    pub end_date: NaiveDateTime,
}

impl NewTicketPricing {
    pub fn validate_record(&self) -> Result<(), DatabaseError> {
        let validation_errors = validators::append_validation_error(
            Ok(()),
            "ticket_pricing.start_date",
            validators::start_date_valid(self.start_date, self.end_date),
        );
        Ok(validation_errors?)
    }

    pub fn commit(self, conn: &PgConnection) -> Result<TicketPricing, DatabaseError> {
        self.validate_record()?;
        diesel::insert_into(ticket_pricing::table)
            .values(self)
            .get_result(conn)
            .to_db_error(ErrorCode::InsertError, "Could not create ticket pricing")
    }
}
