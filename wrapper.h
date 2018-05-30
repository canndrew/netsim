// needed for SIOCADDRT
#include <sys/ioctl.h>

// needed for ifreq
#include <net/if.h>

// needed for lots of symbols
#include <net/if_arp.h>

// needed for rtentry RTF_UP RTF_GATEWAY
#include <net/route.h>


// needed for NLMSG_ALIGNTO ifaddrmsg rtattr RTM_NEWADDR IFA_ADDRESS
#include <linux/rtnetlink.h>

