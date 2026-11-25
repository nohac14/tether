//! Inert SDK test. Allocates and frees the request record without registering it.
#![allow(unsafe_code)]

use super::*;
use ::windows::Win32::NetworkManagement::Dns::{
    DNS_SERVICE_INSTANCE, DnsServiceConstructInstance, DnsServiceFreeInstance, IP6_ADDRESS,
};
use ::windows::core::PCWSTR;
use std::ptr::NonNull;

struct Instance(NonNull<DNS_SERVICE_INSTANCE>);

impl Drop for Instance {
    fn drop(&mut self) {
        // SAFETY: this pointer came from one successful SDK constructor, and
        // no registration was started or second owner created.
        unsafe { DnsServiceFreeInstance(self.0.as_ptr()) };
    }
}

fn wide(value: &str) -> Vec<u16> {
    value.encode_utf16().chain([0]).collect()
}

#[test]
fn windows_constructs_owned_dns_records_without_registering_a_service() {
    let identity = identity();
    for addresses in [
        ["192.0.2.18"].as_slice(),
        ["192.0.2.18", "fd00::18"].as_slice(),
    ] {
        let (inventory, binding) = fixture(vec![adapter("fixture", addresses, 7, 7)], &["fixture"]);
        let plan =
            Advertisement::for_binding(&binding, &inventory, GENERATION, &identity, None).unwrap();
        let scopes = plan.windows_registrations().unwrap();
        let scope = &scopes[0];
        let label = wide(plan.instance_name());
        let host = wide(plan.hostname());
        let txt = plan.txt_records();
        let keys = txt.map(|(key, _)| wide(key));
        let values = txt.map(|(_, value)| wide(value));
        let key_ptrs = keys.each_ref().map(|value| PCWSTR(value.as_ptr()));
        let value_ptrs = values.each_ref().map(|value| PCWSTR(value.as_ptr()));
        let ipv4 = scope
            .ipv4()
            .map(|address| u32::from_ne_bytes(address.octets()));
        let ipv6 = scope.ipv6().map(|address| IP6_ADDRESS {
            IP6Byte: address.octets(),
        });
        // SAFETY: every pointing argument references the corresponding initialized
        // input through this synchronous call. The UTF-16 buffers are NUL terminated
        // and arrays contain exactly the advertised number of properties.
        let instance = Instance(
            NonNull::new(unsafe {
                DnsServiceConstructInstance(
                    PCWSTR(label.as_ptr()),
                    PCWSTR(host.as_ptr()),
                    ipv4.as_ref().map(std::ptr::from_ref),
                    ipv6.as_ref().map(std::ptr::from_ref),
                    scope.port().get(),
                    0,
                    0,
                    u32::try_from(key_ptrs.len()).unwrap(),
                    key_ptrs.as_ptr(),
                    value_ptrs.as_ptr(),
                )
            })
            .expect("DNS allocation failed"),
        );
        // SAFETY: the owner remains in scope, and the SDK constructed this
        // instance from the fully initialized inputs.
        let view = unsafe { instance.0.as_ref() };
        assert_eq!(view.wPort, PORT.get());
        assert_eq!(view.dwInterfaceIndex, 0);
        assert_eq!(view.dwPropertyCount, 3);
        assert!(!view.ip4Address.is_null());
        assert!(!view.keys.is_null());
        assert!(!view.values.is_null());
        assert!(!std::ptr::eq(view.pszHostName.0, host.as_ptr()));
        assert!(!std::ptr::eq(view.pszInstanceName.0, label.as_ptr()));
        // SAFETY: SDK returns copied NUL-terminated strings and exact native
        // address storage while the instance lives.
        unsafe {
            assert_eq!(
                std::slice::from_raw_parts(view.pszHostName.0, host.len()),
                host
            );
            assert_eq!(
                std::slice::from_raw_parts(view.pszInstanceName.0, label.len()),
                label
            );
            for (native, expected) in [(view.keys, &keys), (view.values, &values)] {
                for (stored, original) in std::slice::from_raw_parts(native, expected.len())
                    .iter()
                    .zip(expected)
                {
                    assert!(!stored.0.is_null());
                    assert!(!std::ptr::eq(stored.0, original.as_ptr()));
                    assert_eq!(
                        std::slice::from_raw_parts(stored.0, original.len()),
                        original
                    );
                }
            }
            assert_eq!((*view.ip4Address).to_ne_bytes(), [192, 0, 2, 18]);
            if let Some(expected) = scope.ipv6() {
                assert!(!view.ip6Address.is_null());
                assert_eq!((*view.ip6Address).IP6Byte, expected.octets());
            } else {
                assert!(view.ip6Address.is_null());
            }
        }
    }
}
