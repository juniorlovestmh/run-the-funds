# Pluggy: one-time setup guide

Pluggy exposes Brazilian bank data via an `Item` — a user-consented link
between their bank and your Pluggy application. Creating an `Item` requires
a browser-based consent flow (OAuth-ish) that the CLI can't do. Once the
`Item` exists, rtf uses its `itemId` plus your application's
`clientId` / `clientSecret` to pull transactions indefinitely.

You do this once per bank account (or once per "connection" — a single
`Item` can carry multiple accounts under the same bank login).

## Steps

1. **Register at [pluggy.ai](https://pluggy.ai)** and create an Application.
   The free developer tier is sufficient for personal use.
2. **Copy the credentials** the dashboard shows you:
   - `CLIENT_ID`
   - `CLIENT_SECRET`
3. **Create a Connect Token** for the web widget. From a terminal:

   ```bash
   CLIENT_ID="..."
   CLIENT_SECRET="..."
   API_KEY=$(curl -sS -X POST https://api.pluggy.ai/auth \
     -H 'Content-Type: application/json' \
     -d "{\"clientId\":\"$CLIENT_ID\",\"clientSecret\":\"$CLIENT_SECRET\"}" \
     | jq -r .apiKey)
   CONNECT_TOKEN=$(curl -sS -X POST https://api.pluggy.ai/connect_token \
     -H "X-API-KEY: $API_KEY" -H 'Content-Type: application/json' \
     -d '{}' | jq -r .accessToken)
   echo "Connect token: $CONNECT_TOKEN"
   ```

4. **Open Pluggy's hosted Connect UI** in a browser, using your Connect
   Token as a query param. See the Connect reference in Pluggy's docs for
   the current launch URL; typically something like
   `https://connect.pluggy.ai/?connect_token=<TOKEN>`.

5. **Log into your Brazilian bank** through the widget. On success, Pluggy
   redirects (or shows) the resulting `itemId`. Copy it.

6. **Hand rtf the credentials**:

   ```bash
   rtf pluggy setup \
     --client-id "$CLIENT_ID" \
     --client-secret "$CLIENT_SECRET" \
     --item-id "<itemId-from-step-5>"
   ```

   This stores all three in the local `provider_credentials` table.
   Re-running replaces the stored credentials (rotate keys / switch
   `itemId` safely).

7. **Link your local accounts**. List your Pluggy accounts once to find
   their ids:

   ```bash
   curl -sS "https://api.pluggy.ai/accounts?itemId=<itemId>" \
     -H "X-API-KEY: $API_KEY" | jq '.results[] | {id, name, currencyCode}'
   ```

   Then, for each Brazilian account you want rtf to sync:

   ```bash
   rtf accounts link --id <local-uuid> --provider pluggy --external-id <pluggy-account-uuid>
   ```

8. **Run sync**:

   ```bash
   rtf sync --provider pluggy
   ```

   First sync pulls up to two years of history; subsequent syncs are
   incremental from each account's `last_sync_at`.

## Notes

- **One itemId per bank link.** If you connect a second bank, run the
  Connect flow again and re-run `rtf pluggy setup` with the new
  `itemId`. The previous configuration is replaced — you'll lose sync
  access to the previous bank until you restore that configuration. For
  multi-bank support we need richer credential storage (follow-up).
- **Token rotation.** `clientSecret` rotation: run `rtf pluggy setup`
  again with the new secret.
- **Connect session expiry.** Pluggy's Connect session has a TTL; the
  `itemId` is durable (doesn't expire). Only the widget interaction is
  time-limited.
