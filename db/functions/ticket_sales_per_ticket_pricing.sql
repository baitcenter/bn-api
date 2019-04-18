--------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------
-- TICKET SALES PER TICKET PRICING
--------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------
-- Legacy function call
DROP FUNCTION IF EXISTS ticket_sales_per_ticket_pricing(start TIMESTAMP, "end" TIMESTAMP, group_by_ticket_type BOOLEAN, group_by_event_id BOOLEAN);
DROP FUNCTION IF EXISTS ticket_sales_per_ticket_pricing(start TIMESTAMP, "end" TIMESTAMP, group_by_ticket_type BOOLEAN, group_by_event_id BOOLEAN, group_by_hold_id BOOLEAN);
-- New function call
DROP FUNCTION IF EXISTS ticket_sales_per_ticket_pricing(start TIMESTAMP, "end" TIMESTAMP, group_by TEXT);
-- The group_by can be a combination of 'hold', 'ticket_type', 'ticket_pricing' or a combination: 'ticket_type|ticket_pricing|hold'
-- Default grouping is by event if no sub-group is defined
CREATE OR REPLACE FUNCTION ticket_sales_per_ticket_pricing(start TIMESTAMP, "end" TIMESTAMP, group_by TEXT)
    RETURNS TABLE
    (
        organization_id                    UUID,
        event_id                           UUID,
        ticket_type_id                     UUID,
        ticket_pricing_id                  UUID,
        hold_id                            UUID,
        ticket_name                        TEXT,
        ticket_status                      TEXT,
        event_name                         TEXT,
        hold_name                          TEXT,
        promo_redemption_code              TEXT,
        ticket_pricing_name                TEXT,
        ticket_pricing_price_in_cents      BIGINT,
        promo_code_discounted_ticket_price BIGINT,
        box_office_order_count             BIGINT,
        online_order_count                 BIGINT,
        box_office_sales_in_cents          BIGINT,
        online_sales_in_cents              BIGINT,
        box_office_face_sales_in_cents     BIGINT,
        online_face_sales_in_cents         BIGINT,
        box_office_refunded_count          BIGINT,
        online_refunded_count              BIGINT,
        box_office_sale_count              BIGINT,
        online_sale_count                  BIGINT,
        comp_sale_count                    BIGINT,
        total_box_office_fees_in_cents     BIGINT,
        total_online_fees_in_cents         BIGINT,
        company_box_office_fees_in_cents   BIGINT,
        client_box_office_fees_in_cents    BIGINT,
        company_online_fees_in_cents       BIGINT,
        client_online_fees_in_cents        BIGINT,
        per_order_company_online_fees      BIGINT,
        per_order_client_online_fees       BIGINT,
        per_order_total_fees_in_cents      BIGINT

    )
AS
$body$
SELECT e.organization_id                            AS organization_id,
       e.id                                         AS event_id,
       tt.id                                        AS ticket_type_id,
       tp.id                                        AS ticket_pricing_id,
       COALESCE(gh.id, c.id) AS hold_id,
       tt.name                                      AS ticket_name,
       tt.status                                    AS ticket_status,
       e.name                                       AS event_name,
       COALESCE(gh.name, c.name)                    AS hold_name,--Actually hold or promo code name
       c.redemption_code                            AS promo_redemption_code,
       tp.name                                      AS ticket_pricing_name,
       tp.price_in_cents                            AS ticket_pricing_price_in_cents,
       oi_promo_code_price.unit_price_in_cents      AS promo_code_discounted_ticket_price,
       -- Order count
       CAST(COALESCE(SUM(CASE WHEN o.status = 'Paid' THEN 1 ELSE 0 END)
                         FILTER (WHERE p.is_box_office IS TRUE AND o.status = 'Paid'),
                     0) AS BIGINT)                  AS box_office_order_count,
       CAST(COALESCE(SUM(CASE WHEN o.status = 'Paid' THEN 1 ELSE 0 END)
                         FILTER (WHERE p.is_box_office IS FALSE AND o.status = 'Paid'),
                     0) AS BIGINT)                  AS online_order_count,
       -- Actual Gross Values
       CAST(COALESCE(SUM(
                             (oi.unit_price_in_cents * (oi.quantity - oi.refunded_quantity))
                             + (COALESCE(oi_promo_code.unit_price_in_cents, 0) *
                                (COALESCE(oi_promo_code.quantity, 0) - COALESCE(oi_promo_code.refunded_quantity, 0)))
                         )
                             FILTER (WHERE p.is_box_office IS TRUE AND o.status = 'Paid'),
                     0) AS BIGINT)                  AS box_office_sales_in_cents,
       CAST(COALESCE(SUM(
                             (oi.unit_price_in_cents * (oi.quantity - oi.refunded_quantity))
                             + (COALESCE(oi_promo_code.unit_price_in_cents, 0) *
                                (COALESCE(oi_promo_code.quantity,0) - COALESCE(oi_promo_code.refunded_quantity,0)))
                         )
                             FILTER (WHERE p.is_box_office IS FALSE AND o.status = 'Paid'),
                     0) AS BIGINT)                  AS online_sales_in_cents,

       -- Actual Face Values
       CAST(COALESCE(SUM(oi.unit_price_in_cents * (oi.quantity - oi.refunded_quantity))
                         FILTER (WHERE p.is_box_office IS TRUE AND o.status = 'Paid'),
                     0) AS BIGINT)                  AS box_office_face_sales_in_cents,
       CAST(COALESCE(SUM(oi.unit_price_in_cents * (oi.quantity - oi.refunded_quantity))
                         FILTER (WHERE p.is_box_office IS FALSE AND o.status = 'Paid'),
                     0) AS BIGINT)                  AS online_face_sales_in_cents,

       -- Refunded count
       CAST(COALESCE(SUM(oi.refunded_quantity) FILTER (WHERE p.is_box_office IS TRUE AND o.status = 'Paid'),
                     0) AS BIGINT)                  AS box_office_refunded_count,
       CAST(COALESCE(SUM(oi.refunded_quantity) FILTER (WHERE p.is_box_office IS FALSE AND o.status = 'Paid'),
                     0) AS BIGINT)                  AS online_refunded_count,

       --Total Sold Count
       CAST(COALESCE(SUM(oi.quantity - oi.refunded_quantity)
                         FILTER (WHERE p.is_box_office IS TRUE AND o.status = 'Paid' AND
                                       (h.hold_type IS NULL OR h.hold_type != 'Comp')),
                     0) AS BIGINT)                  AS box_office_sale_count,
       CAST(COALESCE(SUM(oi.quantity - oi.refunded_quantity)
                         FILTER (WHERE p.is_box_office IS FALSE AND o.status = 'Paid' AND
                                       (h.hold_type IS NULL OR h.hold_type != 'Comp')),
                     0) AS BIGINT)                  AS online_sale_count,
       CAST(COALESCE(SUM(oi.quantity - oi.refunded_quantity) FILTER (WHERE h.hold_type = 'Comp' AND o.status = 'Paid'),
                     0) AS BIGINT)                  AS comp_sale_count,


       -- Total box office fees
       CAST(COALESCE(SUM((COALESCE(oi_t_fees.unit_price_in_cents, 0) *
                          (COALESCE(oi_t_fees.quantity, 0) - COALESCE(oi_t_fees.refunded_quantity, 0))))
                         FILTER (WHERE p.is_box_office IS TRUE),
                     0) AS BIGINT)                  AS total_box_office_fees_in_cents,
       -- Total online fees
       CAST(COALESCE(SUM((COALESCE(oi_t_fees.unit_price_in_cents, 0) *
                          (COALESCE(oi_t_fees.quantity, 0) - COALESCE(oi_t_fees.refunded_quantity, 0))))
                         FILTER (WHERE p.is_box_office IS FALSE),
                     0) AS BIGINT)                  AS total_online_fees_in_cents,
       -- Per Ticket Company Box Office Fees
       CAST(COALESCE(SUM((COALESCE(oi_t_fees.company_fee_in_cents, 0) *
                          (COALESCE(oi_t_fees.quantity, 0) - COALESCE(oi_t_fees.refunded_quantity, 0))))
                         FILTER (WHERE p.is_box_office IS TRUE),
                     0) AS BIGINT)                  AS company_box_office_fees_in_cents,
       -- Per Ticket Client Box Office Fees
       CAST(COALESCE(SUM((COALESCE(oi_t_fees.client_fee_in_cents, 0) *
                          (COALESCE(oi_t_fees.quantity, 0) - COALESCE(oi_t_fees.refunded_quantity, 0))))
                         FILTER (WHERE p.is_box_office IS TRUE),
                     0) AS BIGINT)                  AS client_box_office_fees_in_cents,
       -- Per Ticket Company Online Fees
       CAST(COALESCE(SUM((COALESCE(oi_t_fees.company_fee_in_cents, 0) *
                          (COALESCE(oi_t_fees.quantity, 0) - COALESCE(oi_t_fees.refunded_quantity, 0))))
                         FILTER (WHERE p.is_box_office IS FALSE),
                     0) AS BIGINT)                  AS company_online_fees_in_cents,
       -- Per Ticket Client Online Fees
       CAST(COALESCE(SUM((COALESCE(oi_t_fees.client_fee_in_cents, 0) *
                          (COALESCE(oi_t_fees.quantity, 0) - COALESCE(oi_t_fees.refunded_quantity, 0))))
                         FILTER (WHERE p.is_box_office IS FALSE),
                     0) AS BIGINT)                  AS client_online_fees_in_cents,
       --These are calculated by event_fees_per_event and inserted in the code.
       CAST(0 AS BIGINT)                            AS per_order_company_online_fees,
       CAST(0 AS BIGINT)                            AS per_order_client_online_fees,
       CAST(0 AS BIGINT)                            AS per_order_total_fees_in_cents

FROM order_items oi
         LEFT JOIN order_items oi_promo_code
                   ON (oi_promo_code.item_type = 'Discount' AND oi.id = oi_promo_code.parent_id)
         LEFT JOIN (SELECT
                        oi_promo_code_price.unit_price_in_cents,
                        oi_promo_code_price.item_type,
                        oi_promo_code_price.parent_id
                    FROM order_items oi_promo_code_price
                    WHERE $3 LIKE '%hold%') AS oi_promo_code_price
                   ON (oi_promo_code_price.item_type = 'Discount' AND oi.id = oi_promo_code_price.parent_id)
         LEFT JOIN (SELECT c.id, c.name, c.redemption_code FROM codes c WHERE $3 LIKE '%hold%') AS c
                   ON c.id = oi.code_id
         LEFT JOIN orders o on oi.order_id = o.id
         LEFT JOIN events e on oi.event_id = e.id
         LEFT JOIN holds h ON oi.hold_id = h.id
         LEFT JOIN order_items oi_t_fees ON oi_t_fees.parent_id = oi.id AND oi_t_fees.item_type = 'PerUnitFees'
         LEFT JOIN (SELECT order_id,
                           CAST(ARRAY_TO_STRING(ARRAY_AGG(DISTINCT p.payment_method), ', ') LIKE
                                '%External' AS BOOLEAN) AS is_box_office
                    FROM payments p
                    WHERE p.status = 'Completed'
                    GROUP BY p.payment_method, p.order_id) AS p on o.id = p.order_id
         LEFT JOIN (SELECT gh.id, gh.name FROM holds gh WHERE $3 LIKE '%hold%') as gh ON gh.id = oi.hold_id
         LEFT JOIN (SELECT tt.id, tt.name, tt.status FROM ticket_types tt WHERE $3 LIKE '%ticket_type%') AS tt
                   ON tt.id = oi.ticket_type_id
         LEFT JOIN (SELECT tp.id, tp.name, tp.price_in_cents
                    FROM ticket_pricing tp
                    WHERE $3 LIKE '%ticket_pricing%') AS tp ON oi.ticket_pricing_id = tp.id


WHERE oi.ticket_type_id IS NOT NULL
  AND ($1 IS NULL OR o.paid_at >= $1)
  AND ($2 IS NULL OR o.paid_at <= $2)
GROUP BY e.id, tt.id, tt.name, tt.status, tp.id, tp.name, tp.price_in_cents, gh.id, gh.name, c.id, c.name,
         c.redemption_code, oi_promo_code_price.unit_price_in_cents;
$body$
    LANGUAGE SQL;

