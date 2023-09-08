# CatanService
a service that implements a Settlers of Catan game logic

It works best to run the /test in a separate VS Code window (then the launch settings work)

test/test.sh will test the WebAPI
src/cosmos_db.rs has a test that directly tests the cosmos abstraction


Dependencies (required executables that the service uses)

- Azure CLI
- Azure CLI Communications Services Extension
    *had to do this as the installation of the extension did not work: 
    
    /opt/homebrew/Ce/opt/homebrew/Cellar/azure-cli/2.50.0_1/libexec/bin/pip install azure-communication-sms
    /opt/homebrew/Cellar/azure-cli/2.50.0_1/libexec/bin/pip install azure-communication-phonenumbers

/opt/homebrew/Cellar/azure-cli/2.50.0_1/libexec/bin/pip install azure-communication-email

- Python
- 
- 
- az communication sms send --sender +18662361341  --recipient +12069152796 --message "Hey -- this is a test!"

az communication list --resource-group catan-rg --query '[0].name' -o tsv
az communication list-key --resource-group catan-rg --name ct-comm-service --query 'secondaryConnectionString' -o tsv
az communication phonenumber list --query '[0].phoneNumber' -o tsv


az communication list-key --resource-group catan-rg --name ct-comm-service
az communication list --resource-group catan-rg
az communication phonenumber list