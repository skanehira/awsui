![GitHub Repo stars](https://img.shields.io/github/stars/skanehira/awsui?style=social)
![GitHub](https://img.shields.io/github/license/skanehira/awsui)
![GitHub all releases](https://img.shields.io/github/downloads/skanehira/awsui/total)
![GitHub CI Status](https://img.shields.io/github/actions/workflow/status/skanehira/awsui/ci.yaml?branch=main)
![GitHub Release Status](https://img.shields.io/github/v/release/skanehira/awsui)

# awsui

A TUI application for managing AWS resources via SSO.

## Supported Services

- EC2 (Elastic Compute Cloud)
- ECR (Elastic Container Registry)
- ECS (Elastic Container Service)
- S3 (Simple Storage Service)
- VPC (Virtual Private Cloud)
- Secrets Manager

## Requirements

- AWS CLI configured with SSO (`~/.aws/config`)
- macOS or Linux

> **Note**: Currently only AWS SSO authentication is supported. Support for other authentication methods is planned.

## Installation

Download the latest binary from [Releases](https://github.com/skanehira/awsui/releases).

Or build from source:

```bash
cargo install --git https://github.com/skanehira/awsui
```

## Usage

```bash
# Start with interactive profile selection
awsui

# Skip profile selection and use a specific profile
awsui --profile my-profile

# Allow delete operations for all services
awsui --allow-delete

# Allow delete operations for specific services only
awsui --allow-delete ec2,s3
```

## Key Bindings

### Global

| Key                 | Action       |
| ------------------- | ------------ |
| `q` / `Ctrl+c`      | Quit         |
| `?`                 | Show help    |
| `Tab` / `Shift+Tab` | Switch tabs  |
| `Ctrl+t`            | Open new tab |
| `Ctrl+w`            | Close tab    |

### List View

| Key                 | Action               |
| ------------------- | -------------------- |
| `j` / `Down`        | Move down            |
| `k` / `Up`          | Move up              |
| `g` / `G`           | Move to top / bottom |
| `Ctrl+d` / `Ctrl+u` | Half page down / up  |
| `Enter`             | Open detail          |
| `/`                 | Start filter         |
| `r`                 | Refresh              |
| `y`                 | Copy ID              |
| `Esc`               | Back                 |

### Detail View

| Key       | Action                     |
| --------- | -------------------------- |
| `]` / `[` | Next / previous detail tab |
| `j` / `k` | Scroll down / up           |
| `y`       | Copy ID                    |
| `Esc`     | Back                       |

### Service-Specific Keys

| Service         | Key | Action                 |
| --------------- | --- | ---------------------- |
| EC2             | `S` | Start / Stop instance  |
| EC2             | `R` | Reboot instance        |
| EC2             | `D` | Terminate instance     |
| EC2             | `s` | SSM connect            |
| ECS             | `l` | Show logs              |
| ECS             | `a` | ECS Exec               |
| S3              | `c` | Create bucket          |
| S3              | `D` | Delete bucket / object |
| Secrets Manager | `c` | Create secret          |
| Secrets Manager | `D` | Delete secret          |
| Secrets Manager | `e` | Edit secret            |
| Secrets Manager | `v` | Reveal secret value    |
