use bigneon_db::dev::TestProject;
use bigneon_db::models::*;
use bigneon_db::utils::dates;
use bigneon_db::utils::errors::DatabaseError;
use bigneon_db::utils::errors::ErrorCode;
use chrono::prelude::*;
use diesel;
use diesel::sql_types;
use diesel::RunQueryDsl;
use tari_client::*;
use time::Duration;
use uuid::Uuid;

#[test]
fn receive_url() {
    let project = TestProject::new();
    let connection = project.get_connection();
    let user = project.create_user().finish();
    let event = project.create_event().with_ticket_pricing().finish();
    project
        .create_order()
        .for_user(&user)
        .for_event(&event)
        .quantity(1)
        .is_paid()
        .finish();
    let ticket = &TicketInstance::find_for_user(user.id, connection).unwrap()[0];
    let transfer = Transfer::create(user.id, Uuid::new_v4(), None, None, false)
        .commit(connection)
        .unwrap();
    transfer.add_transfer_ticket(ticket.id, connection).unwrap();

    assert_eq!(transfer.receive_url("http://example.com".to_string(), connection).unwrap(),
        format!("http://example.com/tickets/transfers/receive?sender_user_id={}&transfer_key={}&num_tickets=1&signature={}", transfer.source_user_id, transfer.transfer_key, transfer.signature(connection).unwrap()).to_string()
    );
}

#[test]
fn into_authorization() {
    let project = TestProject::new();
    let connection = project.get_connection();
    let user = project.create_user().finish();
    let event = project.create_event().with_ticket_pricing().finish();
    project
        .create_order()
        .for_user(&user)
        .for_event(&event)
        .quantity(1)
        .is_paid()
        .finish();
    let ticket = &TicketInstance::find_for_user(user.id, connection).unwrap()[0];
    let transfer = Transfer::create(user.id, Uuid::new_v4(), None, None, false)
        .commit(connection)
        .unwrap();
    transfer.add_transfer_ticket(ticket.id, connection).unwrap();
    assert_eq!(
        TransferAuthorization {
            transfer_key: transfer.transfer_key,
            sender_user_id: transfer.source_user_id,
            num_tickets: 1,
            signature: transfer.signature(connection).unwrap(),
        },
        transfer.into_authorization(connection).unwrap()
    );
}

#[test]
fn transfer_ticket_count() {
    let project = TestProject::new();
    let connection = project.get_connection();
    let user = project.create_user().finish();
    let event = project.create_event().with_ticket_pricing().finish();
    project
        .create_order()
        .for_user(&user)
        .for_event(&event)
        .quantity(2)
        .is_paid()
        .finish();
    let tickets = TicketInstance::find_for_user(user.id, connection).unwrap();
    let transfer = Transfer::create(user.id, Uuid::new_v4(), None, None, false)
        .commit(connection)
        .unwrap();
    assert_eq!(transfer.transfer_ticket_count(connection).unwrap(), 0);

    transfer
        .add_transfer_ticket(tickets[0].id, connection)
        .unwrap();
    assert_eq!(transfer.transfer_ticket_count(connection).unwrap(), 1);

    transfer
        .add_transfer_ticket(tickets[1].id, connection)
        .unwrap();
    assert_eq!(transfer.transfer_ticket_count(connection).unwrap(), 2);
}

#[test]
fn signature() {
    let project = TestProject::new();
    let connection = project.get_connection();
    let user = project.create_user().finish();
    let event = project.create_event().with_ticket_pricing().finish();
    project
        .create_order()
        .for_user(&user)
        .for_event(&event)
        .quantity(2)
        .is_paid()
        .finish();
    let tickets = TicketInstance::find_for_user(user.id, connection).unwrap();
    let transfer = Transfer::create(user.id, Uuid::new_v4(), None, None, false)
        .commit(connection)
        .unwrap();
    for ticket in tickets {
        transfer.add_transfer_ticket(ticket.id, connection).unwrap();
    }

    let mut message: String = transfer.transfer_key.to_string();
    message.push_str(transfer.source_user_id.to_string().as_str());
    message.push_str(
        (transfer.transfer_ticket_count(connection).unwrap() as u32)
            .to_string()
            .as_str(),
    );
    let wallet = Wallet::find_default_for_user(transfer.source_user_id, connection).unwrap();
    let secret_key = wallet.secret_key;
    let expected_signature = convert_bytes_to_hexstring(
        &cryptographic_signature(&message, &convert_hexstring_to_bytes(&secret_key)).unwrap(),
    );
    let found_signature = transfer.signature(connection).unwrap();
    assert_eq!(expected_signature, found_signature);

    let mut header: String = transfer.transfer_key.to_string();
    header.push_str(transfer.source_user_id.to_string().as_str());
    header.push_str(2.to_string().as_str());
    assert!(cryptographic_verify(
        &convert_hexstring_to_bytes(&found_signature),
        &header,
        &convert_hexstring_to_bytes(&wallet.public_key),
    ));
}

#[test]
fn events_have_not_ended() {
    let project = TestProject::new();
    let connection = project.get_connection();
    let user = project.create_user().finish();
    let event = project.create_event().with_ticket_pricing().finish();
    project
        .create_order()
        .for_user(&user)
        .for_event(&event)
        .quantity(1)
        .is_paid()
        .finish();
    let ticket = &TicketInstance::find_for_user(user.id, connection).unwrap()[0];
    let transfer = Transfer::create(user.id, Uuid::new_v4(), None, None, false)
        .commit(connection)
        .unwrap();
    transfer.add_transfer_ticket(ticket.id, connection).unwrap();
    assert!(transfer.events_have_not_ended(connection).unwrap());

    let parameters = EventEditableAttributes {
        event_start: Some(dates::now().add_days(-2).finish()),
        event_end: Some(dates::now().add_days(-1).finish()),
        ..Default::default()
    };
    event.update(None, parameters, connection).unwrap();

    assert!(!transfer.events_have_not_ended(connection).unwrap());
}

#[test]
fn sender_name() {
    let project = TestProject::new();
    let connection = project.get_connection();
    let user = project
        .create_user()
        .with_first_name("Bob")
        .with_last_name("Miller")
        .finish();
    assert_eq!(Transfer::sender_name(&user), "Bob M.".to_string());

    let user = user
        .update(
            UserEditableAttributes {
                first_name: Some(None),
                last_name: Some(None),
                ..Default::default()
            },
            None,
            connection,
        )
        .unwrap();
    assert_eq!(Transfer::sender_name(&user), "another user".to_string());
}

#[test]
fn update_associated_orders() {
    let project = TestProject::new();
    let connection = project.get_connection();
    let user = project.create_user().finish();
    let user2 = project.create_user().finish();
    let order = project
        .create_order()
        .for_user(&user)
        .quantity(1)
        .is_paid()
        .finish();
    let order2 = project
        .create_order()
        .for_user(&user2)
        .quantity(1)
        .is_paid()
        .finish();
    let ticket = &TicketInstance::find_for_user(user.id, connection).unwrap()[0];
    let ticket2 = &TicketInstance::find_for_user(user2.id, connection).unwrap()[0];

    let transfer = Transfer::create(user.id, Uuid::new_v4(), None, None, false)
        .commit(connection)
        .unwrap();
    transfer.add_transfer_ticket(ticket.id, connection).unwrap();
    assert!(transfer.update_associated_orders(connection).is_ok());
    assert_eq!(vec![transfer], order.transfers(connection).unwrap());

    let transfer2 = Transfer::create(user2.id, Uuid::new_v4(), None, None, false)
        .commit(connection)
        .unwrap();
    transfer2
        .add_transfer_ticket(ticket2.id, connection)
        .unwrap();
    assert!(transfer2.update_associated_orders(connection).is_ok());
    assert_eq!(vec![transfer2], order2.transfers(connection).unwrap());
}

#[test]
fn orders() {
    let project = TestProject::new();
    let connection = project.get_connection();
    let user = project.create_user().finish();
    let order = project
        .create_order()
        .for_user(&user)
        .quantity(1)
        .is_paid()
        .finish();
    let ticket = &TicketInstance::find_for_user(user.id, connection).unwrap()[0];

    let transfer = Transfer::create(user.id, Uuid::new_v4(), None, None, false)
        .commit(connection)
        .unwrap();
    transfer.add_transfer_ticket(ticket.id, connection).unwrap();
    assert!(transfer.update_associated_orders(connection).is_ok());
    assert_eq!(
        vec![Order::find(order.id, connection).unwrap()],
        transfer.orders(connection).unwrap()
    );
}

#[test]
fn transfer_tickets() {
    let project = TestProject::new();
    let connection = project.get_connection();
    let user = project.create_user().finish();
    project
        .create_order()
        .for_user(&user)
        .quantity(2)
        .is_paid()
        .finish();
    let tickets = TicketInstance::find_for_user(user.id, connection).unwrap();
    let ticket = &tickets[0];
    let ticket2 = &tickets[1];

    let transfer = Transfer::create(user.id, Uuid::new_v4(), None, None, false)
        .commit(connection)
        .unwrap();
    let transfer_ticket = transfer.add_transfer_ticket(ticket.id, connection).unwrap();

    let transfer2 = Transfer::create(user.id, Uuid::new_v4(), None, None, false)
        .commit(connection)
        .unwrap();
    let transfer_ticket2 = transfer2
        .add_transfer_ticket(ticket2.id, connection)
        .unwrap();

    assert_eq!(
        vec![transfer_ticket],
        transfer.transfer_tickets(connection).unwrap()
    );
    assert_eq!(
        vec![transfer_ticket2],
        transfer2.transfer_tickets(connection).unwrap()
    );
}

#[test]
fn for_display() {
    let project = TestProject::new();
    let connection = project.get_connection();
    let user = project.create_user().finish();
    let event = project.create_event().with_ticket_pricing().finish();
    project
        .create_order()
        .for_event(&event)
        .for_user(&user)
        .quantity(1)
        .is_paid()
        .finish();
    let ticket = &TicketInstance::find_for_user(user.id, connection).unwrap()[0];

    let transfer = Transfer::create(user.id, Uuid::new_v4(), None, None, false)
        .commit(connection)
        .unwrap();
    transfer.add_transfer_ticket(ticket.id, connection).unwrap();
    let display_transfer = transfer.for_display(connection).unwrap();
    assert_eq!(display_transfer.id, transfer.id);
    assert_eq!(display_transfer.ticket_ids, vec![ticket.id]);
    assert_eq!(display_transfer.event_ids, vec![event.id]);
}

#[test]
fn find_by_user_id() {
    let project = TestProject::new();
    let connection = project.get_connection();
    let user = project.create_user().finish();
    let user2 = project.create_user().finish();
    let user3 = project.create_user().finish();
    let user4 = project.create_user().finish();
    let order = project
        .create_order()
        .for_user(&user)
        .quantity(2)
        .is_paid()
        .finish();
    let order2 = project
        .create_order()
        .for_user(&user2)
        .quantity(1)
        .is_paid()
        .finish();
    let tickets = TicketInstance::find_for_user(user.id, connection).unwrap();
    let ticket = &tickets[0];
    let ticket2 = &TicketInstance::find_for_user(user2.id, connection).unwrap()[0];

    let mut transfer = Transfer::create(user.id, Uuid::new_v4(), None, None, false)
        .commit(connection)
        .unwrap();
    transfer.add_transfer_ticket(ticket.id, connection).unwrap();
    transfer.update_associated_orders(connection).unwrap();
    transfer = transfer
        .update(
            TransferEditableAttributes {
                destination_user_id: Some(user3.id),
                ..Default::default()
            },
            connection,
        )
        .unwrap();
    let transfer = transfer.for_display(connection).unwrap();

    let mut transfer2 = Transfer::create(user2.id, Uuid::new_v4(), None, None, false)
        .commit(connection)
        .unwrap();
    transfer2
        .add_transfer_ticket(ticket2.id, connection)
        .unwrap();
    transfer2.update_associated_orders(connection).unwrap();
    transfer2 = transfer2
        .update(
            TransferEditableAttributes {
                destination_user_id: Some(user4.id),
                ..Default::default()
            },
            connection,
        )
        .unwrap();
    let transfer2 = transfer2.for_display(connection).unwrap();

    // Outgoing
    assert_eq!(
        vec![transfer.clone()],
        Transfer::find_for_user_for_display(
            user.id,
            None,
            SourceOrDestination::Source,
            None,
            None,
            None,
            None,
            connection
        )
        .unwrap()
        .data
    );
    assert_eq!(
        vec![transfer2.clone()],
        Transfer::find_for_user_for_display(
            user2.id,
            None,
            SourceOrDestination::Source,
            None,
            None,
            None,
            None,
            connection
        )
        .unwrap()
        .data
    );
    assert!(Transfer::find_for_user_for_display(
        user3.id,
        None,
        SourceOrDestination::Source,
        None,
        None,
        None,
        None,
        connection
    )
    .unwrap()
    .is_empty());
    assert!(Transfer::find_for_user_for_display(
        user4.id,
        None,
        SourceOrDestination::Source,
        None,
        None,
        None,
        None,
        connection
    )
    .unwrap()
    .is_empty());

    // Incoming
    assert!(Transfer::find_for_user_for_display(
        user.id,
        None,
        SourceOrDestination::Destination,
        None,
        None,
        None,
        None,
        connection
    )
    .unwrap()
    .is_empty());
    assert!(Transfer::find_for_user_for_display(
        user2.id,
        None,
        SourceOrDestination::Destination,
        None,
        None,
        None,
        None,
        connection
    )
    .unwrap()
    .is_empty());
    assert_eq!(
        vec![transfer.clone()],
        Transfer::find_for_user_for_display(
            user3.id,
            None,
            SourceOrDestination::Destination,
            None,
            None,
            None,
            None,
            connection
        )
        .unwrap()
        .data
    );
    assert_eq!(
        vec![transfer2.clone()],
        Transfer::find_for_user_for_display(
            user4.id,
            None,
            SourceOrDestination::Destination,
            None,
            None,
            None,
            None,
            connection
        )
        .unwrap()
        .data
    );

    // Lookup specific to order
    assert_eq!(
        vec![transfer.clone()],
        Transfer::find_for_user_for_display(
            user.id,
            Some(order.id),
            SourceOrDestination::Source,
            None,
            None,
            None,
            None,
            connection
        )
        .unwrap()
        .data
    );
    assert!(Transfer::find_for_user_for_display(
        user2.id,
        Some(order.id),
        SourceOrDestination::Source,
        None,
        None,
        None,
        None,
        connection
    )
    .unwrap()
    .is_empty());
    assert!(Transfer::find_for_user_for_display(
        user3.id,
        Some(order.id),
        SourceOrDestination::Source,
        None,
        None,
        None,
        None,
        connection
    )
    .unwrap()
    .is_empty());
    assert!(Transfer::find_for_user_for_display(
        user4.id,
        Some(order.id),
        SourceOrDestination::Source,
        None,
        None,
        None,
        None,
        connection
    )
    .unwrap()
    .is_empty());
    assert!(Transfer::find_for_user_for_display(
        user.id,
        Some(order2.id),
        SourceOrDestination::Source,
        None,
        None,
        None,
        None,
        connection
    )
    .unwrap()
    .is_empty());
    assert_eq!(
        vec![transfer2.clone()],
        Transfer::find_for_user_for_display(
            user2.id,
            Some(order2.id),
            SourceOrDestination::Source,
            None,
            None,
            None,
            None,
            connection
        )
        .unwrap()
        .data
    );
    assert!(Transfer::find_for_user_for_display(
        user3.id,
        Some(order2.id),
        SourceOrDestination::Source,
        None,
        None,
        None,
        None,
        connection
    )
    .unwrap()
    .is_empty());
    assert!(Transfer::find_for_user_for_display(
        user4.id,
        Some(order2.id),
        SourceOrDestination::Source,
        None,
        None,
        None,
        None,
        connection
    )
    .unwrap()
    .is_empty());

    // Pagination
    let ticket3 = &tickets[1];
    let transfer3 = Transfer::create(user.id, Uuid::new_v4(), None, None, false)
        .commit(connection)
        .unwrap();
    transfer3
        .add_transfer_ticket(ticket3.id, connection)
        .unwrap();
    transfer3.update_associated_orders(connection).unwrap();
    let transfer3 = transfer3.for_display(connection).unwrap();

    assert_eq!(
        vec![transfer.clone()],
        Transfer::find_for_user_for_display(
            user.id,
            None,
            SourceOrDestination::Source,
            None,
            None,
            Some(1),
            Some(0),
            connection
        )
        .unwrap()
        .data
    );
    assert_eq!(
        vec![transfer3.clone()],
        Transfer::find_for_user_for_display(
            user.id,
            None,
            SourceOrDestination::Source,
            None,
            None,
            Some(1),
            Some(1),
            connection
        )
        .unwrap()
        .data
    );

    // Limit by start date
    let date = Utc::now().naive_utc() - Duration::minutes(30);
    let before_date = date - Duration::minutes(35);
    let after_date = date + Duration::minutes(35);
    diesel::sql_query(
        r#"
        UPDATE transfers
        SET created_at = $1
        WHERE id = $2;
        "#,
    )
    .bind::<sql_types::Timestamp, _>(date)
    .bind::<sql_types::Uuid, _>(transfer3.id)
    .execute(connection)
    .unwrap();
    let transfer3 = Transfer::find(transfer3.id, connection)
        .unwrap()
        .for_display(connection)
        .unwrap();
    assert_eq!(
        vec![transfer.clone(), transfer3.clone()],
        Transfer::find_for_user_for_display(
            user.id,
            None,
            SourceOrDestination::Source,
            Some(before_date),
            None,
            None,
            None,
            connection
        )
        .unwrap()
        .data
    );
    assert_eq!(
        vec![transfer.clone(), transfer3.clone()],
        Transfer::find_for_user_for_display(
            user.id,
            None,
            SourceOrDestination::Source,
            Some(date),
            None,
            None,
            None,
            connection
        )
        .unwrap()
        .data
    );
    assert!(Transfer::find_for_user_for_display(
        user.id,
        None,
        SourceOrDestination::Source,
        Some(after_date),
        None,
        None,
        None,
        connection
    )
    .unwrap()
    .is_empty());

    // Limit by end date
    assert_eq!(
        vec![transfer.clone(), transfer3.clone()],
        Transfer::find_for_user_for_display(
            user.id,
            None,
            SourceOrDestination::Source,
            None,
            Some(after_date),
            None,
            None,
            connection
        )
        .unwrap()
        .data
    );
    assert_eq!(
        vec![transfer3.clone()],
        Transfer::find_for_user_for_display(
            user.id,
            None,
            SourceOrDestination::Source,
            None,
            Some(date),
            None,
            None,
            connection
        )
        .unwrap()
        .data
    );
    assert!(Transfer::find_for_user_for_display(
        user.id,
        None,
        SourceOrDestination::Source,
        None,
        Some(before_date),
        None,
        None,
        connection
    )
    .unwrap()
    .is_empty());
}

#[test]
fn find() {
    let project = TestProject::new();
    let connection = project.get_connection();
    let user = project.create_user().finish();
    project.create_order().for_user(&user).is_paid().finish();
    let transfer = Transfer::create(user.id, Uuid::new_v4(), None, None, false)
        .commit(connection)
        .unwrap();

    assert_eq!(transfer, Transfer::find(transfer.id, connection).unwrap());
}

#[test]
fn find_by_transfer_key() {
    let project = TestProject::new();
    let connection = project.get_connection();
    let user = project.create_user().finish();
    project.create_order().for_user(&user).is_paid().finish();
    let ticket = &TicketInstance::find_for_user(user.id, connection).unwrap()[0];
    let transfer_key = Uuid::new_v4();
    let transfer = Transfer::create(user.id, transfer_key.clone(), None, None, false)
        .commit(connection)
        .unwrap();
    transfer.add_transfer_ticket(ticket.id, connection).unwrap();

    let found_transfer = Transfer::find_by_transfer_key(transfer_key, connection).unwrap();
    assert_eq!(found_transfer, transfer);
}

#[test]
fn add_transfer_ticket() {
    let project = TestProject::new();
    let connection = project.get_connection();
    let user = project.create_user().finish();
    project
        .create_order()
        .for_user(&user)
        .quantity(1)
        .is_paid()
        .finish();
    let ticket = &TicketInstance::find_for_user(user.id, connection).unwrap()[0];
    let transfer = Transfer::create(user.id, Uuid::new_v4(), None, None, false)
        .commit(connection)
        .unwrap();
    let transfer_ticket = transfer.add_transfer_ticket(ticket.id, connection).unwrap();
    assert_eq!(transfer_ticket.transfer_id, transfer.id);
    assert_eq!(transfer_ticket.ticket_instance_id, ticket.id);
}

#[test]
fn find_pending() {
    let project = TestProject::new();
    let connection = project.get_connection();
    let user = project.create_user().finish();
    let user2 = project.create_user().finish();
    project
        .create_order()
        .for_user(&user)
        .quantity(1)
        .is_paid()
        .finish();
    let tickets = TicketInstance::find_for_user(user.id, connection).unwrap();
    let ticket = &tickets[0];

    let transfer = Transfer::create(user.id, Uuid::new_v4(), None, None, false)
        .commit(connection)
        .unwrap();
    transfer.add_transfer_ticket(ticket.id, connection).unwrap();
    let pending_transfers = Transfer::find_pending(connection).unwrap();
    assert_eq!(pending_transfers.len(), 1);
    assert_eq!(pending_transfers[0].id, transfer.id);

    // Complete transfer to remove from result set
    assert!(transfer.complete(user2.id, None, connection).is_ok());
    assert_eq!(Transfer::find_pending(connection).unwrap().len(), 0);

    // New transfer still pending
    let transfer2 = Transfer::create(user.id, Uuid::new_v4(), None, None, false)
        .commit(connection)
        .unwrap();
    transfer2
        .add_transfer_ticket(ticket.id, connection)
        .unwrap();

    let pending_transfers = Transfer::find_pending(connection).unwrap();
    assert_eq!(pending_transfers.len(), 1);
    assert_eq!(pending_transfers[0].id, transfer2.id);
}

#[test]
fn find_pending_by_ticket_instance_ids() {
    let project = TestProject::new();
    let connection = project.get_connection();
    let user = project.create_user().finish();
    let user2 = project.create_user().finish();
    project
        .create_order()
        .for_user(&user)
        .quantity(2)
        .is_paid()
        .finish();
    let tickets = TicketInstance::find_for_user(user.id, connection).unwrap();
    let ticket = &tickets[0];
    let ticket2 = &tickets[1];

    let transfer = Transfer::create(user.id, Uuid::new_v4(), None, None, false)
        .commit(connection)
        .unwrap();
    transfer.add_transfer_ticket(ticket.id, connection).unwrap();

    assert!(transfer.complete(user2.id, None, connection).is_ok());
    let transfer2 = Transfer::create(user.id, Uuid::new_v4(), None, None, false)
        .commit(connection)
        .unwrap();
    transfer2
        .add_transfer_ticket(ticket.id, connection)
        .unwrap();

    let pending_transfers =
        Transfer::find_pending_by_ticket_instance_ids(&[ticket.id, ticket2.id], connection)
            .unwrap();
    assert_eq!(pending_transfers.len(), 1);
    assert_eq!(pending_transfers[0].id, transfer2.id);
}

#[test]
fn cancel() {
    let project = TestProject::new();
    let connection = project.get_connection();
    let user = project.create_user().finish();
    project
        .create_order()
        .for_user(&user)
        .quantity(1)
        .is_paid()
        .finish();
    let ticket = TicketInstance::find_for_user(user.id, connection)
        .unwrap()
        .remove(0);
    let transfer_key = Uuid::new_v4();
    let transfer = Transfer::create(user.id, transfer_key, None, None, false)
        .commit(connection)
        .unwrap();
    transfer.add_transfer_ticket(ticket.id, connection).unwrap();

    let domain_events = DomainEvent::find(
        Tables::Transfers,
        Some(transfer.id),
        Some(DomainEventTypes::TransferTicketCancelled),
        connection,
    )
    .unwrap();
    assert_eq!(0, domain_events.len());

    let transfer = transfer.cancel(user.id, None, connection).unwrap();
    assert_eq!(transfer.status, TransferStatus::Cancelled);
    let domain_events = DomainEvent::find(
        Tables::Transfers,
        Some(transfer.id),
        Some(DomainEventTypes::TransferTicketCancelled),
        connection,
    )
    .unwrap();
    assert_eq!(1, domain_events.len());

    // Transfering again triggers error as status is no longer pending
    let result = transfer.cancel(user.id, None, connection);
    assert!(result.is_err());
    assert_eq!(
        result.unwrap_err(),
        DatabaseError::new(
            ErrorCode::UpdateError,
            Some("Transfer cannot be cancelled as it is no longer pending".to_string()),
        )
    );
}

#[test]
fn complete() {
    let project = TestProject::new();
    let connection = project.get_connection();
    let user = project.create_user().finish();
    let user2 = project.create_user().finish();
    project
        .create_order()
        .for_user(&user)
        .quantity(1)
        .is_paid()
        .finish();
    let ticket = TicketInstance::find_for_user(user.id, connection)
        .unwrap()
        .remove(0);
    let transfer_key = Uuid::new_v4();
    let transfer = Transfer::create(user.id, transfer_key, None, None, false)
        .commit(connection)
        .unwrap();
    transfer.add_transfer_ticket(ticket.id, connection).unwrap();

    let domain_events = DomainEvent::find(
        Tables::Transfers,
        Some(transfer.id),
        Some(DomainEventTypes::TransferTicketCompleted),
        connection,
    )
    .unwrap();
    assert_eq!(0, domain_events.len());

    let transfer = transfer.complete(user2.id, None, connection).unwrap();
    assert_eq!(transfer.status, TransferStatus::Completed);
    assert_eq!(transfer.destination_user_id, Some(user2.id));
    let domain_events = DomainEvent::find(
        Tables::Transfers,
        Some(transfer.id),
        Some(DomainEventTypes::TransferTicketCompleted),
        connection,
    )
    .unwrap();
    assert_eq!(1, domain_events.len());

    // Transfering again triggers error as status is no longer pending
    let result = transfer.complete(user2.id, None, connection);
    assert!(result.is_err());
    assert_eq!(
        result.unwrap_err(),
        DatabaseError::new(
            ErrorCode::UpdateError,
            Some("Transfer cannot be completed as it is no longer pending".to_string()),
        )
    );
}

#[test]
fn create_commit() {
    let project = TestProject::new();
    let connection = project.get_connection();
    let user = project.create_user().finish();
    project
        .create_order()
        .for_user(&user)
        .quantity(1)
        .is_paid()
        .finish();
    let ticket = TicketInstance::find_for_user(user.id, connection)
        .unwrap()
        .remove(0);
    let transfer_key = Uuid::new_v4();

    let transfer = Transfer::create(user.id, transfer_key, None, None, false)
        .commit(connection)
        .unwrap();
    transfer.add_transfer_ticket(ticket.id, connection).unwrap();
    assert_eq!(transfer.status, TransferStatus::Pending);
    assert_eq!(transfer.source_user_id, user.id);
    assert_eq!(transfer.transfer_key, transfer_key);
}

#[test]
fn update() {
    let project = TestProject::new();
    let connection = project.get_connection();
    let user = project.create_user().finish();
    project
        .create_order()
        .for_user(&user)
        .quantity(1)
        .is_paid()
        .finish();
    let ticket = TicketInstance::find_for_user(user.id, connection)
        .unwrap()
        .remove(0);
    let transfer_key = Uuid::new_v4();
    let transfer = Transfer::create(user.id, transfer_key.clone(), None, None, false)
        .commit(connection)
        .unwrap();
    transfer.add_transfer_ticket(ticket.id, connection).unwrap();

    let transfer = transfer
        .update(
            TransferEditableAttributes {
                status: Some(TransferStatus::Cancelled),
                ..Default::default()
            },
            connection,
        )
        .unwrap();
    assert_eq!(transfer.status, TransferStatus::Cancelled);
}
