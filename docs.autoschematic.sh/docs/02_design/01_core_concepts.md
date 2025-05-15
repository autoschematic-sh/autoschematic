# Autoschematic: Core Concepts

Autoschematic is a platform for building declarative ops systems. This means that
Autoschematic fills the same role as "infrastructure-as-code" (IaC) tools like Terraform, Ansible and Cloudformation.
These tools are _declarative_ because they let you express the desired state of a system or resource, without worrying about
the steps needed to reach that state.

Autoschematic is designed to meet the needs of large organizations. It was developed 
based on several years of experience in a DevOps consulting context in order to better serve
the complex "real-world" requirements that we encountered at many organizations. 

These companies had large amounts of existing cloud infra that was not managed by IaC.
They had complex security and auditing requirements, such as SOC 2 and PCI-DSS. 
They had many accounts and teams, and used interconnected resources across accounts and cloud providers. 
Furthermore, these companies were all struggling to serve their internal users' requests for infrastructure
provisioning. The journey in introducing IaC and best practices at these companies, and in carrying out 
platform engineering for them, led to a desire for a tool that would have a lower barrier of entry while still
retaining the critical advantages of IaC.

Autoschematic was designed from the ground up, taking a fundamentally different approach compared to existing tools.
This approach can be summarized by the following 4 principles:
1. Resource addressing by file hierarchy
2. Connector modularity and isolation
3. Bidirectional state modelling
4. Git as central to the state model

## How is "desired state" expressed in Autoschematic?
Autoschematic integrates tightly with Git, and with Github, in order to provide a smooth working model for teams to collaborate and agree on the desired state of a system. 
This model has a number of advantages. Any change that anyone has made via Autoschematic is traceable to a Github Pull Request, and undoing an errant change is as simple as rolling back or reverting the commit(s) in question.

Indeed, a model can be enforced using Github where team members who work on 
sensitive cloud infrastructure do not need broad access credentials. In such a model, they can draft desired changes using Autoschematic, but enacting those changes is blocked until another 
team member reviews those changes and grants an approval. This is more desirable than a model that 
many organizations adopt, whereby many individual team members have nearly admin-level access,
and must each tread lightly, and operate the AWS dashboard with great care.

An Autoschematic repository is a Git repository containing Autoschematic configuration and resources.
The most important aspect of this configuration is the set of _prefixes_. Each prefix might represent a 
separate team, or office, or sub-organization. You can decide to create your prefixes in any manner you choose, such as:

```
├── autoschematic.ron
└── offices
    ├── london
    ├── sanfrancisco
    └── singapore
```
`autoschematic.ron` is the root configuration file, and must always be present at the root of the repository. In this case, it contains:
```
AutoschematicConfig(
    prefixes: {
        "offices/london": [
            //Nothing in here, yet!
        ],
        "offices/sanfrancisco": [
        ],
        "offices/singapore": [
        ],
    }
)
```
(N.B.: For more information about the ron format, see [https://github.com/ron-rs/ron](https://github.com/ron-rs/ron))

Within each prefix, one or more connectors are defined. 
Connectors are the core plugin concept for autoschematic. Each connector manages one or more classes of resources, including:

#### Defining an address mapping from the file hierarchy within a repo/prefix to some real resource
For instance, suppose that our prefix `offices/london` has their own AWS account.
With the AWS Connector installed in that prefix, we might have a file in the repository at

`offices/london/aws/s3/eu-west-2/tps-reports.ron`

Here, the `aws/s3/...` portion of the path represents the address of the S3Bucket resource, orthogonal to the `offices/london` prefix.

Each file path uniquely identifies a single resource within a prefix. Some resources may logically "belong" to other resources. Consider Route53 records belonging to a hosted zone:

```
offices/london/aws/route53/hosted_zones/datadyne.corp/
├── config.ron 
└── records
    ├── A
    │   ├── datadyne.corp.ron
    │   └── backend.datadyne.corp.ron
    ├── CNAME
    │   ├── _5c8b8d4259981552569ad2cced18cdb.datadyne.corp.ron
    │   └── _65c34af66180a3de0eadfddbe0e0a55.backend.datadyne.corp.ron
    ├── MX
    │   └── datadyne.corp.ron
    ├── NS
    │   └── datadyne.corp.ron
    ├── SOA
    │   └── datadyne.corp.ron
    └── TXT
        └── _acme-challenge.datadyne.corp.ron
```

Here, `aws/route53/hosted_zones/datadyne.corp/config.ron` represents the actual hosted zone definition for a Route53 Hosted Zone called "datadyne.corp". The resource record sets under `records` belong to that hosted zone, and the connector will correctly work all of this out when it decodes a given file path.

#### Defining the on-disk representation of a resource
Let's take a look at the contents of the S3 bucket we mentioned.

```
#![enable(implicit_some)]
S3Bucket(
    policy: None,
    public_access_block: PublicAccessBlock(
        block_public_acls: true,
        ignore_public_acls: true,
        block_public_policy: true,
        restrict_public_buckets: true,
    ),
    acl: Acl(
        owner_id: "<redacted>",
        grants: [
            Grant(
                grantee_id: "<redacted>",
                permission: "FULL_CONTROL",
            ),
        ],
    ),
    tags: Tags({
        "project": "xmas_party",
        "classification": "confidential"
    }),
)
```
Critically, connectors can choose whatever on-disk representation they deem most suitable for the application at hand. For example, the Snowflake connector,
to manage databases, schemas, and tables in Snowflake, uses plain old SQL with Snowflake's Data Definition Language. This is a big deal!


#### Defining a set of operations that can be performed on a resource ('Ops')
In order to manipulate resources, we categorize a set of operations that are performed on those resources. 
For instance, if we want to change the tags on our S3 bucket, we can modify the file 
#### Determining the set of operations that must be performed in order to go from the current state to some desired state
#### Executing those operations and returning structured output data


