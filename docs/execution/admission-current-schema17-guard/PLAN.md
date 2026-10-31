# Admission current-schema fixture guard

Keep the existing real admission profile, reserved-input drift refusal, original lifecycle limits, one-attempt charge and active-budget cleanup checks. The fixture creates a current Store, so its typed user_version guard must require current17. Change only that value and run source checks; Root executes the unchanged actual admission scenario and obtains independent review.
