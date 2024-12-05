CHAIN_ID=testgaia
ADDR_TO_NAME_MAP_FILE="/tmp/addr_to_name_map.txt"

sleep 3 # trying to deal with runtime error: invalid memory address or nil pointer dereference: panic
echo "Creating validators..."

# Create a temporary file to store the address-to-name map
gaiad keys list --keyring-backend=test --home=/opt --output json | jq -r '.[] | .address + " " + .name' > "$ADDR_TO_NAME_MAP_FILE"

# Query the validators
gaiad query staking validators --output json | jq -r '.validators | .[] | .operator_address' | while read -r VAL_ADDRESS; do
    # Extract the key address
    KEY_ADDRESS="cosmos${VAL_ADDRESS#cosmosvaloper}"
    KEY_ADDRESS=${KEY_ADDRESS%??????} # Remove last 6 characters

    # Find the name corresponding to the key address
    KEY_NAME=$(grep "^$KEY_ADDRESS " "$ADDR_TO_NAME_MAP_FILE" | awk '{print $2}')

    if [ -n "$KEY_NAME" ]; then
        gaiad tx staking validator-bond "$VAL_ADDRESS" --from "$KEY_NAME" --chain-id $CHAIN_ID --home=/opt --keyring-backend=test -y >> /opt/gaiad.log 2>&1
        sleep 2
    else
        echo "No key name found for address: $KEY_ADDRESS"
    fi
done

# Clean up the temporary file
rm -f "$ADDR_TO_NAME_MAP_FILE"
