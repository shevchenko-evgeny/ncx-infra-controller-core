/*
 * SPDX-FileCopyrightText: Copyright (c) 2026 NVIDIA CORPORATION & AFFILIATES. All rights reserved.
 * SPDX-License-Identifier: Apache-2.0
 *
 * Licensed under the Apache License, Version 2.0 (the "License");
 * you may not use this file except in compliance with the License.
 * You may obtain a copy of the License at
 *
 * http://www.apache.org/licenses/LICENSE-2.0
 *
 * Unless required by applicable law or agreed to in writing, software
 * distributed under the License is distributed on an "AS IS" BASIS,
 * WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied.
 * See the License for the specific language governing permissions and
 * limitations under the License.
 */

use rpc::forge::forge_server::Forge;
use tonic::Code;

use crate::tests::common::api_fixtures::create_test_env;

#[crate::sqlx_test]
async fn test_create_operating_system_ipxe(pool: sqlx::PgPool) {
    let env = create_test_env(pool).await;

    let resp = env
        .api
        .create_operating_system(tonic::Request::new(rpc::forge::CreateOperatingSystemRequest {
            id: None,
            name: "test-ipxe-os".to_string(),
            org: "test-org".to_string(),
            description: Some("inline iPXE OS".to_string()),
            is_active: true,
            allow_override: true,
            phone_home_enabled: false,
            user_data: Some("cloud-init data".to_string()),
            ipxe_script: Some("chain --autofree https://boot.netboot.xyz".to_string()),
            ipxe_template_name: None,
            ipxe_parameters: vec![],
            ipxe_artifacts: vec![],
        }))
        .await
        .unwrap();

    let os = resp.into_inner();
    assert_eq!(os.name, "test-ipxe-os");
    assert_eq!(os.org, "test-org");
    assert_eq!(os.r#type, "iPXE");
    assert_eq!(os.description.as_deref(), Some("inline iPXE OS"));
    assert!(os.is_active);
    assert!(os.allow_override);
    assert!(!os.phone_home_enabled);
    assert_eq!(os.user_data.as_deref(), Some("cloud-init data"));
    assert_eq!(
        os.ipxe_script.as_deref(),
        Some("chain --autofree https://boot.netboot.xyz")
    );
    assert!(os.ipxe_template_name.is_none());
    assert!(!os.id.is_empty());
}

#[crate::sqlx_test]
async fn test_create_operating_system_requires_name(pool: sqlx::PgPool) {
    let env = create_test_env(pool).await;

    let resp = env
        .api
        .create_operating_system(tonic::Request::new(rpc::forge::CreateOperatingSystemRequest {
            id: None,
            name: "".to_string(),
            org: "test-org".to_string(),
            description: None,
            is_active: true,
            allow_override: true,
            phone_home_enabled: false,
            user_data: None,
            ipxe_script: Some("chain http://example.com".to_string()),
            ipxe_template_name: None,
            ipxe_parameters: vec![],
            ipxe_artifacts: vec![],
        }))
        .await;

    assert!(resp.is_err());
    assert_eq!(resp.unwrap_err().code(), Code::InvalidArgument);
}

#[crate::sqlx_test]
async fn test_create_operating_system_requires_variant(pool: sqlx::PgPool) {
    let env = create_test_env(pool).await;

    let resp = env
        .api
        .create_operating_system(tonic::Request::new(rpc::forge::CreateOperatingSystemRequest {
            id: None,
            name: "test-os".to_string(),
            org: "test-org".to_string(),
            description: None,
            is_active: true,
            allow_override: true,
            phone_home_enabled: false,
            user_data: None,
            ipxe_script: None,
            ipxe_template_name: None,
            ipxe_parameters: vec![],
            ipxe_artifacts: vec![],
        }))
        .await;

    assert!(resp.is_err());
    assert_eq!(resp.unwrap_err().code(), Code::InvalidArgument);
}

#[crate::sqlx_test]
async fn test_get_operating_system(pool: sqlx::PgPool) {
    let env = create_test_env(pool).await;

    let created = env
        .api
        .create_operating_system(tonic::Request::new(rpc::forge::CreateOperatingSystemRequest {
            id: None,
            name: "get-test-os".to_string(),
            org: "org1".to_string(),
            description: None,
            is_active: true,
            allow_override: true,
            phone_home_enabled: false,
            user_data: None,
            ipxe_script: Some("chain http://boot.example.com".to_string()),
            ipxe_template_name: None,
            ipxe_parameters: vec![],
            ipxe_artifacts: vec![],
        }))
        .await
        .unwrap()
        .into_inner();

    let id: rpc::common::Uuid = uuid::Uuid::parse_str(&created.id).unwrap().into();

    let fetched = env
        .api
        .get_operating_system(tonic::Request::new(id))
        .await
        .unwrap()
        .into_inner();

    assert_eq!(fetched.id, created.id);
    assert_eq!(fetched.name, "get-test-os");
    assert_eq!(fetched.r#type, "iPXE");
}

#[crate::sqlx_test]
async fn test_get_operating_system_not_found(pool: sqlx::PgPool) {
    let env = create_test_env(pool).await;

    let id: rpc::common::Uuid = uuid::Uuid::nil().into();
    let resp = env
        .api
        .get_operating_system(tonic::Request::new(id))
        .await;

    assert!(resp.is_err());
    assert_eq!(resp.unwrap_err().code(), Code::NotFound);
}

#[crate::sqlx_test]
async fn test_update_operating_system(pool: sqlx::PgPool) {
    let env = create_test_env(pool).await;

    let created = env
        .api
        .create_operating_system(tonic::Request::new(rpc::forge::CreateOperatingSystemRequest {
            id: None,
            name: "original-name".to_string(),
            org: "org1".to_string(),
            description: Some("original desc".to_string()),
            is_active: true,
            allow_override: false,
            phone_home_enabled: false,
            user_data: None,
            ipxe_script: Some("chain http://example.com".to_string()),
            ipxe_template_name: None,
            ipxe_parameters: vec![],
            ipxe_artifacts: vec![],
        }))
        .await
        .unwrap()
        .into_inner();

    let id: rpc::common::Uuid = uuid::Uuid::parse_str(&created.id).unwrap().into();

    let updated = env
        .api
        .update_operating_system(tonic::Request::new(rpc::forge::UpdateOperatingSystemRequest {
            id: Some(id),
            name: Some("updated-name".to_string()),
            description: Some("updated desc".to_string()),
            is_active: Some(false),
            allow_override: Some(true),
            phone_home_enabled: Some(true),
            user_data: Some("new user-data".to_string()),
            ipxe_script: Some("chain http://updated.example.com".to_string()),
            ipxe_template_name: None,
            ipxe_parameters: vec![],
            ipxe_artifacts: vec![],
        }))
        .await
        .unwrap()
        .into_inner();

    assert_eq!(updated.name, "updated-name");
    assert_eq!(updated.description.as_deref(), Some("updated desc"));
    assert!(!updated.is_active);
    assert!(updated.allow_override);
    assert!(updated.phone_home_enabled);
    assert_eq!(updated.user_data.as_deref(), Some("new user-data"));
    assert_eq!(
        updated.ipxe_script.as_deref(),
        Some("chain http://updated.example.com")
    );
}

#[crate::sqlx_test]
async fn test_delete_operating_system(pool: sqlx::PgPool) {
    let env = create_test_env(pool).await;

    let created = env
        .api
        .create_operating_system(tonic::Request::new(rpc::forge::CreateOperatingSystemRequest {
            id: None,
            name: "delete-test-os".to_string(),
            org: "org1".to_string(),
            description: None,
            is_active: true,
            allow_override: true,
            phone_home_enabled: false,
            user_data: None,
            ipxe_script: Some("chain http://example.com".to_string()),
            ipxe_template_name: None,
            ipxe_parameters: vec![],
            ipxe_artifacts: vec![],
        }))
        .await
        .unwrap()
        .into_inner();

    let id: rpc::common::Uuid = uuid::Uuid::parse_str(&created.id).unwrap().into();

    let del_resp = env
        .api
        .delete_operating_system(tonic::Request::new(id.clone().into()))
        .await;
    assert!(del_resp.is_ok());

    let get_resp = env
        .api
        .get_operating_system(tonic::Request::new(id))
        .await;
    assert!(get_resp.is_err());
    assert_eq!(get_resp.unwrap_err().code(), Code::NotFound);
}

#[crate::sqlx_test]
async fn test_find_operating_system_ids(pool: sqlx::PgPool) {
    let env = create_test_env(pool).await;

    let os1 = env
        .api
        .create_operating_system(tonic::Request::new(rpc::forge::CreateOperatingSystemRequest {
            id: None,
            name: "find-os-1".to_string(),
            org: "find-org".to_string(),
            description: None,
            is_active: true,
            allow_override: true,
            phone_home_enabled: false,
            user_data: None,
            ipxe_script: Some("chain http://one.example.com".to_string()),
            ipxe_template_name: None,
            ipxe_parameters: vec![],
            ipxe_artifacts: vec![],
        }))
        .await
        .unwrap()
        .into_inner();

    let os2 = env
        .api
        .create_operating_system(tonic::Request::new(rpc::forge::CreateOperatingSystemRequest {
            id: None,
            name: "find-os-2".to_string(),
            org: "find-org".to_string(),
            description: None,
            is_active: true,
            allow_override: true,
            phone_home_enabled: false,
            user_data: None,
            ipxe_script: Some("chain http://two.example.com".to_string()),
            ipxe_template_name: None,
            ipxe_parameters: vec![],
            ipxe_artifacts: vec![],
        }))
        .await
        .unwrap()
        .into_inner();

    let resp = env
        .api
        .find_operating_system_ids(tonic::Request::new(
            rpc::forge::OperatingSystemSearchFilter {
                org: Some("find-org".to_string()),
            },
        ))
        .await
        .unwrap()
        .into_inner();

    let ids: Vec<String> = resp.ids.iter().map(|u| u.value.clone()).collect();
    assert!(ids.contains(&os1.id));
    assert!(ids.contains(&os2.id));
    assert_eq!(ids.len(), 2);
}

#[crate::sqlx_test]
async fn test_find_operating_systems_by_ids(pool: sqlx::PgPool) {
    let env = create_test_env(pool).await;

    let os1 = env
        .api
        .create_operating_system(tonic::Request::new(rpc::forge::CreateOperatingSystemRequest {
            id: None,
            name: "by-id-os-1".to_string(),
            org: "org1".to_string(),
            description: None,
            is_active: true,
            allow_override: true,
            phone_home_enabled: false,
            user_data: None,
            ipxe_script: Some("chain http://one.example.com".to_string()),
            ipxe_template_name: None,
            ipxe_parameters: vec![],
            ipxe_artifacts: vec![],
        }))
        .await
        .unwrap()
        .into_inner();

    let id1: rpc::common::Uuid = uuid::Uuid::parse_str(&os1.id).unwrap().into();

    let resp = env
        .api
        .find_operating_systems_by_ids(tonic::Request::new(
            rpc::forge::OperatingSystemsByIdsRequest {
                ids: vec![id1],
            },
        ))
        .await
        .unwrap()
        .into_inner();

    assert_eq!(resp.operating_systems.len(), 1);
    assert_eq!(resp.operating_systems[0].name, "by-id-os-1");
}

#[crate::sqlx_test]
async fn test_list_ipxe_script_templates(pool: sqlx::PgPool) {
    let env = create_test_env(pool).await;

    let resp = env
        .api
        .list_ipxe_script_templates(tonic::Request::new(
            rpc::forge::ListIpxeScriptTemplatesRequest {},
        ))
        .await
        .unwrap()
        .into_inner();

    assert!(
        !resp.templates.is_empty(),
        "should have at least one embedded template"
    );
    for tmpl in &resp.templates {
        assert!(!tmpl.name.is_empty());
        assert!(!tmpl.template.is_empty());
    }
}

#[crate::sqlx_test]
async fn test_get_ipxe_script_template(pool: sqlx::PgPool) {
    let env = create_test_env(pool).await;

    let all = env
        .api
        .list_ipxe_script_templates(tonic::Request::new(
            rpc::forge::ListIpxeScriptTemplatesRequest {},
        ))
        .await
        .unwrap()
        .into_inner();

    let first_name = &all.templates[0].name;

    let resp = env
        .api
        .get_ipxe_script_template(tonic::Request::new(
            rpc::forge::GetIpxeScriptTemplateRequest {
                name: first_name.clone(),
            },
        ))
        .await
        .unwrap()
        .into_inner();

    assert_eq!(&resp.name, first_name);
    assert!(!resp.template.is_empty());
}

#[crate::sqlx_test]
async fn test_get_ipxe_script_template_not_found(pool: sqlx::PgPool) {
    let env = create_test_env(pool).await;

    let resp = env
        .api
        .get_ipxe_script_template(tonic::Request::new(
            rpc::forge::GetIpxeScriptTemplateRequest {
                name: "nonexistent-template".to_string(),
            },
        ))
        .await;

    assert!(resp.is_err());
    assert_eq!(resp.unwrap_err().code(), Code::NotFound);
}

#[crate::sqlx_test]
async fn test_create_operating_system_with_explicit_id(pool: sqlx::PgPool) {
    let env = create_test_env(pool).await;

    let explicit_id = uuid::Uuid::new_v4();
    let id_proto: rpc::common::Uuid = explicit_id.into();

    let resp = env
        .api
        .create_operating_system(tonic::Request::new(rpc::forge::CreateOperatingSystemRequest {
            id: Some(id_proto),
            name: "explicit-id-os".to_string(),
            org: "org1".to_string(),
            description: None,
            is_active: true,
            allow_override: true,
            phone_home_enabled: false,
            user_data: None,
            ipxe_script: Some("chain http://example.com".to_string()),
            ipxe_template_name: None,
            ipxe_parameters: vec![],
            ipxe_artifacts: vec![],
        }))
        .await
        .unwrap()
        .into_inner();

    assert_eq!(resp.id, explicit_id.to_string());
}

#[crate::sqlx_test]
async fn test_deleted_os_not_returned_by_find_ids(pool: sqlx::PgPool) {
    let env = create_test_env(pool).await;

    let created = env
        .api
        .create_operating_system(tonic::Request::new(rpc::forge::CreateOperatingSystemRequest {
            id: None,
            name: "soon-deleted-os".to_string(),
            org: "del-org".to_string(),
            description: None,
            is_active: true,
            allow_override: true,
            phone_home_enabled: false,
            user_data: None,
            ipxe_script: Some("chain http://example.com".to_string()),
            ipxe_template_name: None,
            ipxe_parameters: vec![],
            ipxe_artifacts: vec![],
        }))
        .await
        .unwrap()
        .into_inner();

    let id: rpc::common::Uuid = uuid::Uuid::parse_str(&created.id).unwrap().into();
    env.api
        .delete_operating_system(tonic::Request::new(id.into()))
        .await
        .unwrap();

    let resp = env
        .api
        .find_operating_system_ids(tonic::Request::new(
            rpc::forge::OperatingSystemSearchFilter {
                org: Some("del-org".to_string()),
            },
        ))
        .await
        .unwrap()
        .into_inner();

    assert!(resp.ids.is_empty());
}
