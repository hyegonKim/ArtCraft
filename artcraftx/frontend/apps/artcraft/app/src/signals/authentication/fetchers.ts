import { authentication } from "./authentication";
import { UsersApi } from "~/Classes/ApiManager/UsersApi";
import { BillingApi } from "~/Classes/ApiManager/BillingApi";
import { gtagLogin } from "@storyteller/google-analytics";

import {
  updateActiveSubscriptions,
  updateAuthStatus,
  updateUserInfo,
  setLogoutStates,
} from "./utilities";
import { AUTH_STATUS } from "~/enums";

// NB: Login/signup/logout flows live on the Tauri side now. This module only
// reads the current session and mirrors it into the authentication signals.

export const persistLogin = async () => {
  //Only run First Load, return if not
  if (authentication.status.value !== AUTH_STATUS.INIT) {
    return;
  }
  getUserInfoAndSubcriptions();
};

// NB: Only for SyncStorytellerApiConfig.
export const forceGetUserInfoAndSubcriptions = async () => {
  getUserInfoAndSubcriptions();
};

async function getUserInfoAndSubcriptions() {
  console.log('getUserInfoAndSubcriptions()')
  updateAuthStatus(AUTH_STATUS.GET_USER_INFO);
  const usersApi = new UsersApi();
  const sessionResponse = await usersApi.GetSession();
  if (
    !sessionResponse.success ||
    !sessionResponse.data ||
    !sessionResponse.data.user
  ) {
    setLogoutStates();
    return;
  }

  if (sessionResponse.data && !sessionResponse.data.user.can_access_studio) {
    updateAuthStatus(AUTH_STATUS.NO_ACCESS);
    return;
  }
  
  const userToken = sessionResponse.data.user.user_token;
  if (!!userToken) {
    gtagLogin(userToken);
  }

  const billingApi = new BillingApi();
  const subscriptionsResponse = await billingApi.ListActiveSubscriptions();
  if (
    !subscriptionsResponse.success ||
    !subscriptionsResponse.data ||
    !subscriptionsResponse.data.active_subscriptions
  ) {
    setLogoutStates();
    return;
  }

  updateUserInfo(sessionResponse.data.user);
  updateActiveSubscriptions({
    maybe_loyalty_program: subscriptionsResponse.data.maybe_loyalty_program,
    active_subscriptions: subscriptionsResponse.data.active_subscriptions || [],
  });
  updateAuthStatus(AUTH_STATUS.LOGGED_IN);
}
