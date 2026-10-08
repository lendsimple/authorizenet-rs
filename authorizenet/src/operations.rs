//! The API's operations, grouped by resource.
//!
//! [`operation_table!`] is the single list of every operation: its group, method name,
//! request and response types, and documentation. Other code is generated from it by
//! passing a callback macro, so the clients' methods and the request-response pairing
//! cannot drift apart.

/// Invokes `$callback!` with the operation table, preceded by `[args]`.
///
/// The table is a list of groups:
///
/// ```text
/// group_method => HandleType {
///     /// docs
///     method(RequestType) -> ResponseType;
/// }
/// ```
macro_rules! operation_table {
    ($callback:ident $(, $arg:tt)*) => {
        $callback! {
            [$($arg),*]
            account_updater_jobs => AccountUpdaterJobs {
                /// Details of each card updated or deleted by the Account Updater
                /// process in a month, up to 1000 per request; use paging for more.
                get_details(GetAuJobDetailsRequest) -> GetAuJobDetailsResponse;
                /// A summary of the Account Updater process's results for a month.
                get_summary(GetAuJobSummaryRequest) -> GetAuJobSummaryResponse;
            }
            batches => Batches {
                /// Statistics for a single batch.
                get_statistics(GetBatchStatisticsRequest) -> GetBatchStatisticsResponse;
                /// Settled batches in a date range (by default the last 24 hours),
                /// optionally with statistics.
                list_settled(GetSettledBatchListRequest) -> GetSettledBatchListResponse;
            }
            customer_profiles => CustomerProfiles {
                /// Creates a customer profile, with any payment profiles and shipping
                /// addresses.
                create(CreateCustomerProfileRequest) -> CreateCustomerProfileResponse;
                /// Creates a customer profile, payment profile and shipping address from
                /// an existing successful transaction.
                create_from_transaction(CreateCustomerProfileFromTransactionRequest) -> CreateCustomerProfileResponse;
                /// Charges, refunds or voids using a stored customer profile.
                create_transaction(CreateCustomerProfileTransactionRequest) -> CreateCustomerProfileTransactionResponse;
                /// Deletes a customer profile with all its payment profiles and shipping
                /// addresses.
                delete(DeleteCustomerProfileRequest) -> DeleteCustomerProfileResponse;
                /// A customer profile with its payment profiles and shipping addresses.
                get(GetCustomerProfileRequest) -> GetCustomerProfileResponse;
                /// The ids of every customer profile.
                get_ids(GetCustomerProfileIdsRequest) -> GetCustomerProfileIdsResponse;
                /// Updates a customer profile.
                update(UpdateCustomerProfileRequest) -> UpdateCustomerProfileResponse;
            }
            customer_payment_profiles => CustomerPaymentProfiles {
                /// Adds a payment profile to a customer profile.
                create(CreateCustomerPaymentProfileRequest) -> CreateCustomerPaymentProfileResponse;
                /// Deletes a payment profile from a customer profile.
                delete(DeleteCustomerPaymentProfileRequest) -> DeleteCustomerPaymentProfileResponse;
                /// A payment profile of a customer profile.
                get(GetCustomerPaymentProfileRequest) -> GetCustomerPaymentProfileResponse;
                /// A payment nonce for a payment profile.
                get_nonce(GetCustomerPaymentProfileNonceRequest) -> GetCustomerPaymentProfileNonceResponse;
                /// Payment profiles matching a search, such as cards expiring this month;
                /// 10 per request, with paging for more.
                list(GetCustomerPaymentProfileListRequest) -> GetCustomerPaymentProfileListResponse;
                /// Updates a payment profile.
                update(UpdateCustomerPaymentProfileRequest) -> UpdateCustomerPaymentProfileResponse;
                /// Verifies a payment profile with a test transaction. No receipt email
                /// is sent.
                validate(ValidateCustomerPaymentProfileRequest) -> ValidateCustomerPaymentProfileResponse;
            }
            customer_shipping_addresses => CustomerShippingAddresses {
                /// Adds a shipping address to a customer profile.
                create(CreateCustomerShippingAddressRequest) -> CreateCustomerShippingAddressResponse;
                /// Deletes a shipping address from a customer profile.
                delete(DeleteCustomerShippingAddressRequest) -> DeleteCustomerShippingAddressResponse;
                /// A shipping address of a customer profile.
                get(GetCustomerShippingAddressRequest) -> GetCustomerShippingAddressResponse;
                /// Updates a shipping address.
                update(UpdateCustomerShippingAddressRequest) -> UpdateCustomerShippingAddressResponse;
            }
            hosted_pages => HostedPages {
                /// A form token for the Accept Hosted payment page.
                get_payment_page(GetHostedPaymentPageRequest) -> GetHostedPaymentPageResponse;
                /// A token for the hosted customer profile page.
                get_profile_page(GetHostedProfilePageRequest) -> GetHostedProfilePageResponse;
            }
            merchants => Merchants {
                /// Merchant details, useful for OAuth and Accept integrations.
                get(GetMerchantDetailsRequest) -> GetMerchantDetailsResponse;
                /// Updates merchant details.
                update(UpdateMerchantDetailsRequest) -> UpdateMerchantDetailsResponse;
            }
            misc => Misc {
                /// Decrypts payment data, such as a Visa Checkout payload.
                decrypt_payment_data(DecryptPaymentDataRequest) -> DecryptPaymentDataResponse;
                /// Checks that the API is available. Sent without credentials.
                is_alive(IsAliveRequest) -> IsAliveResponse;
                /// Ends a mobile device session.
                logout(LogoutRequest) -> LogoutResponse;
                /// Checks that the credentials are valid.
                test_authenticate(AuthenticateTestRequest) -> AuthenticateTestResponse;
            }
            mobile_devices => MobileDevices {
                /// The Acceptance Device serial number for a merchant and mobile device.
                get_acceptance_device(GetAdDetailsRequest) -> GetAdDetailsResponse;
                /// Requests a PIN for mobile device login.
                login_pin(MobileDeviceLoginPinRequest) -> MobileDeviceLoginPinResponse;
                /// Verifies a login challenge.
                login_verify_challenge(MobileDeviceLoginVerifyChallengeRequest) -> MobileDeviceLoginVerifyChallengeResponse;
                /// Verifies a login PIN.
                login_verify_pin(MobileDeviceLoginVerifyPinRequest) -> MobileDeviceLoginVerifyPinResponse;
                /// Logs in a mobile device with multi-factor authentication.
                mfa_login(MobileDeviceMfaLoginRequest) -> MobileDeviceMfaLoginResponse;
                /// Registers a mobile device.
                register(MobileDeviceRegistrationRequest) -> MobileDeviceRegistrationResponse;
                /// Saves the Acceptance Device serial number for a merchant and mobile
                /// device.
                save_acceptance_device(SaveDeviceSnRequest) -> SaveDeviceSnResponse;
            }
            secure_payment_containers => SecurePaymentContainers {
                /// Creates a secure payment container, returning opaque payment data.
                create(SecurePaymentContainerRequest) -> SecurePaymentContainerResponse;
            }
            subscriptions => Subscriptions {
                /// Cancels a subscription.
                cancel(ArbCancelSubscriptionRequest) -> ArbCancelSubscriptionResponse;
                /// Creates a recurring billing subscription.
                create(ArbCreateSubscriptionRequest) -> ArbCreateSubscriptionResponse;
                /// A subscription.
                get(ArbGetSubscriptionRequest) -> ArbGetSubscriptionResponse;
                /// A subscription's status.
                get_status(ArbGetSubscriptionStatusRequest) -> ArbGetSubscriptionStatusResponse;
                /// Subscriptions matching a search.
                list(ArbGetSubscriptionListRequest) -> ArbGetSubscriptionListResponse;
                /// Updates a subscription.
                update(ArbUpdateSubscriptionRequest) -> ArbUpdateSubscriptionResponse;
            }
            transactions => Transactions {
                /// Creates a transaction: authorize, capture, charge, refund, void, ...
                create(CreateTransactionRequest) -> CreateTransactionResponse;
                /// The details of a transaction.
                get(GetTransactionDetailsRequest) -> GetTransactionDetailsResponse;
                /// A lightweight summary of a transaction.
                get_summary(GetTransactionSummaryRequest) -> GetTransactionSummaryResponse;
                /// The transactions in a batch, up to 1000 per request; use paging for
                /// more.
                list(GetTransactionListRequest) -> GetTransactionListResponse;
                /// The transactions of a customer profile or payment profile, up to 1000
                /// per request; use paging for more.
                list_for_customer(GetTransactionListForCustomerRequest) -> GetTransactionListResponse;
                /// Unsettled transactions, up to 1000 per request; use paging for more.
                list_unsettled(GetUnsettledTransactionListRequest) -> GetUnsettledTransactionListResponse;
                /// Emails a receipt for a transaction to the customer.
                send_receipt(SendCustomerTransactionReceiptRequest) -> SendCustomerTransactionReceiptResponse;
                /// Approves or declines a held transaction.
                update_held(UpdateHeldTransactionRequest) -> UpdateHeldTransactionResponse;
                /// Updates the status of an order made of several transactions with the
                /// same split tender id.
                update_split_tender_group(UpdateSplitTenderGroupRequest) -> UpdateSplitTenderGroupResponse;
            }
        }
    };
}

#[cfg(any(feature = "async", feature = "blocking"))]
pub(crate) use operation_table;

/// Implements `ApiRequest` for each request in the table.
macro_rules! impl_api_requests {
    ([] $(
        $group:ident => $handle:ident {
            $(
                $(#[$doc:meta])*
                $method:ident($request:ident) -> $response:ident;
            )*
        }
    )*) => {
        $($(
            impl $crate::api::ApiRequest for $crate::schema::$request {
                type Response = $crate::schema::$response;
                const OPERATION: &'static str = concat!(stringify!($group), ".", stringify!($method));
            }
        )*)*
    };
}

operation_table!(impl_api_requests);

/// A client's resource handles and its methods returning them, one per group.
///
/// Arguments: `[Client, async]` or `[Client, blocking]`. `Client` must have an
/// `execute` method, async or not.
#[cfg(any(feature = "async", feature = "blocking"))]
macro_rules! resource_handles {
    (
        [$client:ident, $mode:ident]
        $(
            $group:ident => $handle:ident {
                $(
                    $(#[$doc:meta])*
                    $method:ident($request:ident) -> $response:ident;
                )*
            }
        )*
    ) => {
        impl $client {
            $(
                #[doc = concat!("The `", stringify!($group), "` operations.")]
                pub fn $group(&self) -> $handle<'_> {
                    $handle { client: self }
                }
            )*
        }

        $(
            #[doc = concat!(
                "The `", stringify!($group), "` operations, from [`", stringify!($client),
                "::", stringify!($group), "`]."
            )]
            #[derive(Debug, Clone, Copy)]
            pub struct $handle<'a> {
                client: &'a $client,
            }

            impl $handle<'_> {
                $(
                    resource_handles!(@method $mode $method $request $response $(#[$doc])*);
                )*
            }
        )*
    };

    (@method $mode:ident $method:ident $request:ident $response:ident $(#[$doc:meta])*) => {
        resource_handles!(@sig $mode $method $request $response
            $(#[$doc])*
            #[doc = ""]
            #[doc = concat!(
                "Sends a [`", stringify!($request), "`](crate::schema::", stringify!($request),
                ") and returns its [`", stringify!($response), "`](crate::schema::",
                stringify!($response), ")."
            )]
        );
    };

    (@sig async $method:ident $request:ident $response:ident $(#[$doc:meta])*) => {
        $(#[$doc])*
        pub async fn $method(
            &self,
            request: &$crate::schema::$request,
        ) -> ::core::result::Result<$crate::schema::$response, $crate::Error> {
            self.client.execute(request).await
        }
    };

    (@sig blocking $method:ident $request:ident $response:ident $(#[$doc:meta])*) => {
        $(#[$doc])*
        pub fn $method(
            &self,
            request: &$crate::schema::$request,
        ) -> ::core::result::Result<$crate::schema::$response, $crate::Error> {
            self.client.execute(request)
        }
    };
}

#[cfg(any(feature = "async", feature = "blocking"))]
pub(crate) use resource_handles;
