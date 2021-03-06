CREATE TABLE ticket_instances (
  id uuid PRIMARY KEY DEFAULT gen_random_uuid() NOT NULL,
  asset_id uuid NOT NULL REFERENCES assets(id),
  token_id INT NOT NULL,
  hold_id Uuid NULL REFERENCES holds(id),
  order_item_id Uuid NULL REFERENCES order_items(id),
  wallet_id Uuid NOT NULL REFERENCES wallets(id),
  reserved_until TIMESTAMP NULL,
  redeem_key Text NULL,
  transfer_key Uuid NULL,
  transfer_expiry_date TIMESTAMP NULL,
  status TEXT NOT NULL DEFAULT 'Available',
  created_at TIMESTAMP NOT NULL DEFAULT now(),
  updated_at TIMESTAMP NOT NULL DEFAULT now()
);

-- Indices
CREATE INDEX index_ticket_instances_asset_id ON ticket_instances(asset_id);
CREATE INDEX index_ticket_instances_order_item_id ON ticket_instances(order_item_id);
CREATE INDEX index_ticket_instances_hold_id ON ticket_instances(hold_id);
CREATE INDEX index_ticket_instances_asset_id_token_id ON ticket_instances(asset_id, token_id);
CREATE INDEX index_ticket_instances_redeem_key  ON ticket_instances(redeem_key);
CREATE INDEX index_ticket_instances_wallet_id ON ticket_instances(wallet_id);
CREATE INDEX index_ticket_instances_transfer_key ON ticket_instances(transfer_key);

