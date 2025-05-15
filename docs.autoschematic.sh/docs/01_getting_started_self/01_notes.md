# Notes on Self-Hosting Autoschematic

Autoschematic can be fully self-hosted, enabling a completely open-source platform on 
which to manage your infrastructure. However, a great deal of care needs to be taken with this 
approach. Self-hosting Autoschematic should only be implemented by a qualified DevOps engineer. 

## Security Concerns
### Key Storage
Autoschematic uses a sophisticated secret-sealing system to allow encrypted secrets to be 
stored in Git repositories. However, the root of this system is the keystore. This keystore needs
to be on a secure volume that no other users can access. 

The threat model in Autoschematic's secret sealing system is similar to that of Mozilla's SOPS. 
If the keystore is compromised, it is akin to compromising all sealed secret values within repositories managed by that Autoschematic instance.

### Server endpoint security
While Autoschematic ships with a robust web server and authenticates all requests from Github against a webhook secret,
Autoschematic must not be run without a 

### Cross-region/AZ redundancy
The instructions given are for a single node setup. If greater redundancy is necessary, you may wish to 
scale up your deployment and run multiple instances behind a load-balancer.
This is currently out of scope for this guide and should be implemented by a qualified DevOps engineer experienced
in high-reliability systems and with your own cloud infrastructure. 